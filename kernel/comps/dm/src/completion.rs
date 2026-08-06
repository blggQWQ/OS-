// SPDX-License-Identifier: MPL-2.0

//! Defers mapper completion work out of block-device interrupt callbacks.

use alloc::{boxed::Box, sync::Arc};

use aster_block::bio::BioStatus;
use spin::{Mutex, Once};

/// A one-shot mapper completion job prepared before lower BIO submission.
pub type CompletionJob = Box<dyn FnOnce() + Send + 'static>;

/// A preallocated work item that can be scheduled from interrupt context.
pub trait CompletionWork: Send + Sync {
    /// Schedules the work item for execution in task context.
    fn schedule(&self);
}

/// Creates completion work before the lower device owns the BIO.
pub trait CompletionWorkFactory: Send + Sync {
    /// Creates a one-shot work item without scheduling it.
    fn create(&self, job: CompletionJob) -> Arc<dyn CompletionWork>;
}

static COMPLETION_WORK_FACTORY: Once<Arc<dyn CompletionWorkFactory>> = Once::new();

/// Registers the kernel provider used to defer dm completion work.
///
/// Returns the factory unchanged if a provider has already been registered.
pub fn register_completion_work_factory(
    factory: Arc<dyn CompletionWorkFactory>,
) -> Result<(), Arc<dyn CompletionWorkFactory>> {
    if COMPLETION_WORK_FACTORY.get().is_some() {
        return Err(factory);
    }
    COMPLETION_WORK_FACTORY.call_once(|| factory);
    Ok(())
}

type CompletionJobWithStatus = Box<dyn FnOnce(BioStatus) + Send + 'static>;

/// Couples a lower BIO status with work prepared before submission.
pub(super) enum CompletionDispatch {
    Deferred {
        status: Arc<Mutex<Option<BioStatus>>>,
        work: Arc<dyn CompletionWork>,
    },
    Inline(CompletionJobWithStatus),
}

impl CompletionDispatch {
    /// Prepares all memory needed by the eventual interrupt callback.
    pub(super) fn new<F>(complete_fn: F) -> Self
    where
        F: FnOnce(BioStatus) + Send + 'static,
    {
        let Some(factory) = COMPLETION_WORK_FACTORY.get() else {
            // Component-level tests do not boot the kernel worker provider.
            return Self::Inline(Box::new(complete_fn));
        };

        let status = Arc::new(Mutex::new(None));
        let worker_status = Arc::clone(&status);
        let job = Box::new(move || {
            let status = worker_status.lock().take();
            let Some(status) = status else {
                return;
            };
            complete_fn(status);
        });
        let work = factory.create(job);
        Self::Deferred { status, work }
    }

    /// Publishes the lower status and schedules the preallocated work item.
    pub(super) fn complete(self, lower_status: BioStatus) {
        match self {
            Self::Deferred { status, work } => {
                *status.lock() = Some(lower_status);
                work.schedule();
            }
            Self::Inline(complete_fn) => complete_fn(lower_status),
        }
    }
}
