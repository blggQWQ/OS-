// SPDX-License-Identifier: MPL-2.0

//! Task-context completion queue for Device Mapper crypt requests.

use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicBool, Ordering};

use aster_dm::{CompletionJob, CompletionWork, CompletionWorkFactory};
use intrusive_collections::{LinkedList, LinkedListAtomicLink, intrusive_adapter};
use ostd::sync::{SpinLock, WaitQueue};

use crate::thread::kernel_thread::ThreadOptions;

struct DmCompletionQueue {
    // The intrusive list stores links inside work items, so IRQ-side enqueue
    // never needs to resize or allocate a separate queue node.
    pending: SpinLock<LinkedList<DmCompletionWorkAdapter>>,
    // Producers only wake this queue after releasing `pending`; the consumer
    // can therefore sleep without holding an IRQ-disabling spin lock.
    wait_queue: WaitQueue,
}

struct DmCompletionWork {
    // `Option::take` gives the one-shot job a single execution point even if a
    // buggy lower layer attempts to signal completion more than once.
    job: SpinLock<Option<CompletionJob>>,
    queue: Weak<DmCompletionQueue>,
    weak_self: Weak<Self>,
    // This guards the intrusive link against being inserted twice at once.
    is_pending: AtomicBool,
    link: LinkedListAtomicLink,
}

intrusive_adapter!(DmCompletionWorkAdapter = Arc<DmCompletionWork>: DmCompletionWork {
    link: LinkedListAtomicLink
});

struct DmCompletionWorkFactory {
    queue: Arc<DmCompletionQueue>,
}

impl DmCompletionQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: SpinLock::new(LinkedList::new(DmCompletionWorkAdapter::NEW)),
            wait_queue: WaitQueue::new(),
        })
    }

    fn enqueue(&self, work: Arc<DmCompletionWork>) {
        // Each work item owns its intrusive link, so queue insertion cannot
        // grow a container or allocate while the VirtIO interrupt is disabled.
        let should_wake = {
            let mut pending = self.pending.disable_irq().lock();
            let was_empty = pending.is_empty();
            pending.push_back(work);
            was_empty
        };
        // Once the worker is draining a non-empty queue, further wakeups carry
        // no information and only add atomic/waker traffic on the IRQ path.
        if should_wake {
            self.wait_queue.wake_one();
        }
    }

    fn dequeue(&self) -> Option<Arc<DmCompletionWork>> {
        self.pending.disable_irq().lock().pop_front()
    }

    fn run(self: Arc<Self>) {
        loop {
            let work = self.wait_queue.wait_until(|| self.dequeue());
            work.run();
            // Amortize the wait/wakeup path across a burst. `dequeue` drops the
            // IRQ-safe queue lock before `run` performs crypto or completes BIOs.
            while let Some(work) = self.dequeue() {
                work.run();
            }
        }
    }
}

impl DmCompletionWork {
    fn new(queue: &Arc<DmCompletionQueue>, job: CompletionJob) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            job: SpinLock::new(Some(job)),
            queue: Arc::downgrade(queue),
            weak_self: weak_self.clone(),
            is_pending: AtomicBool::new(false),
            link: LinkedListAtomicLink::new(),
        })
    }

    fn run(&self) {
        // Clear the queue-membership flag before running arbitrary completion
        // code, and run that code after dropping every queue/job spin lock.
        self.is_pending.store(false, Ordering::Release);
        let job = self.job.disable_irq().lock().take();
        if let Some(job) = job {
            job();
        }
    }
}

impl CompletionWork for DmCompletionWork {
    fn schedule(&self) {
        if self
            .is_pending
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        let Some(work) = self.weak_self.upgrade() else {
            return;
        };
        queue.enqueue(work);
    }
}

impl CompletionWorkFactory for DmCompletionWorkFactory {
    fn create(&self, job: CompletionJob) -> Arc<dyn CompletionWork> {
        DmCompletionWork::new(&self.queue, job)
    }
}

pub(super) fn init_in_first_kthread() {
    let queue = DmCompletionQueue::new();
    let worker_queue = Arc::clone(&queue);
    ThreadOptions::new(move || worker_queue.run()).spawn();
    let result =
        aster_dm::register_completion_work_factory(Arc::new(DmCompletionWorkFactory { queue }));
    assert!(result.is_ok());
    ostd::early_println!("[dm] completion worker ready");
}
