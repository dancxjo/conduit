//! Cooperative single-core timing exclusion, not resource admission authority.
//!
//! Every timing-sensitive provider and blocking actor must share this instance.
//! `Cell` deliberately makes it !Sync: no interrupt/core may access it concurrently.
//! Guards may span cooperative awaits; dropping a cancelled future releases them.
use super::CooperativeYield;
use core::{
    cell::Cell,
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};

#[derive(Default)]
pub struct NativeActivity {
    timing: Cell<usize>,
    blocking: Cell<bool>,
}
impl NativeActivity {
    pub const fn new() -> Self {
        Self {
            timing: Cell::new(0),
            blocking: Cell::new(false),
        }
    }
    /// Poll already-authorized work only in synchronous blocking windows.
    /// Each guard is dropped before a pending carrier or cooperative yield can
    /// suspend. This supplies timing exclusion, not resource admission, work
    /// bounds, cancellation, or permission to invoke any provider.
    ///
    /// The caller's `pause` must yield/wake or otherwise permit timing providers
    /// to progress; an immediately-ready no-op cannot resolve timing contention.
    pub async fn with_blocking_polls<F: Future, Y: CooperativeYield>(
        &self,
        future: F,
        yield_to_host: &Y,
    ) -> F::Output {
        let mut future = pin!(future);
        loop {
            let result = poll_fn(|cx| {
                let Some(_guard) = self.try_blocking() else {
                    return Poll::Ready(None);
                };
                future.as_mut().poll(cx).map(Some)
            })
            .await;
            match result {
                Some(output) => return output,
                None => yield_to_host.pause().await,
            }
        }
    }
    /// Multiple timing operations may coexist; blocking work excludes them all.
    pub fn try_timing(&self) -> Option<TimingActivity<'_>> {
        if self.blocking.get() {
            return None;
        }
        let next = self.timing.get().checked_add(1)?;
        self.timing.set(next);
        Some(TimingActivity(self))
    }
    /// Exclusive, nonrecursive blocking window. The caller must separately hold
    /// the exact admitted provider/operation and enforce its finite work bound.
    pub fn try_blocking(&self) -> Option<BlockingActivity<'_>> {
        if self.blocking.get() || self.timing.get() != 0 {
            return None;
        }
        self.blocking.set(true);
        Some(BlockingActivity(self))
    }
}
#[must_use = "dropping the guard releases timing exclusion"]
pub struct TimingActivity<'a>(&'a NativeActivity);
impl Drop for TimingActivity<'_> {
    fn drop(&mut self) {
        self.0.timing.set(self.0.timing.get() - 1);
    }
}
#[must_use = "dropping the guard releases the blocking window"]
pub struct BlockingActivity<'a>(&'a NativeActivity);
impl Drop for BlockingActivity<'_> {
    fn drop(&mut self) {
        self.0.blocking.set(false);
    }
}

#[cfg(test)]
#[path = "native_activity_tests.rs"]
mod tests;
