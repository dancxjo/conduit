use crate::Error;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard,
};
#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    commit: Mutex<()>,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancellationState>);
impl Cancellation {
    /// Cancellation and the final semantic/durable publication share one gate.
    /// If publication already holds it, that commit wins; cancellation applies
    /// to subsequent work. Candidate computation never holds this gate.
    pub fn cancel(&self) {
        let _guard = self.0.commit.lock().unwrap_or_else(|e| e.into_inner());
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub(crate) fn commit_guard(&self) -> Result<MutexGuard<'_, ()>, Error> {
        let guard = self.0.commit.lock().unwrap_or_else(|e| e.into_inner());
        if self.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(guard)
    }
}
