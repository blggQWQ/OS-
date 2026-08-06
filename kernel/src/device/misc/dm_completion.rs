// SPDX-License-Identifier: MPL-2.0

//! Task-context completion queue for Device Mapper crypt requests.

use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicBool, Ordering};

use aster_dm::{CompletionJob, CompletionWork, CompletionWorkFactory};
use intrusive_collections::{LinkedList, LinkedListAtomicLink, intrusive_adapter};
use ostd::sync::{SpinLock, WaitQueue};

use crate::thread::kernel_thread::ThreadOptions;

struct DmCompletionQueue {
    pending: SpinLock<LinkedList<DmCompletionWorkAdapter>>,
    wait_queue: WaitQueue,
}

struct DmCompletionWork {
    job: SpinLock<Option<CompletionJob>>,
    queue: Weak<DmCompletionQueue>,
    weak_self: Weak<Self>,
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
        self.pending.disable_irq().lock().push_back(work);
        self.wait_queue.wake_one();
    }

    fn dequeue(&self) -> Option<Arc<DmCompletionWork>> {
        self.pending.disable_irq().lock().pop_front()
    }

    fn run(self: Arc<Self>) {
        loop {
            let work = self.wait_queue.wait_until(|| self.dequeue());
            work.run();
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
