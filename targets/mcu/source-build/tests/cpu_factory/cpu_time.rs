//! Deterministic platform clock; the original Conduit schedulers remain intact.
use std::future::{poll_fn, Future};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::task::Poll;
static NOW: AtomicU64 = AtomicU64::new(17);
pub fn now_ms() -> u64 {
    NOW.load(SeqCst)
}
pub fn advance(ms: u64) {
    NOW.fetch_add(ms, SeqCst);
}
pub struct Duration(u64);
impl Duration {
    pub fn from_secs(seconds: u64) -> Self {
        Self(seconds.checked_mul(1000).unwrap())
    }
}
pub struct Timer {
    deadline: u64,
}
impl Timer {
    pub fn after_millis(ms: u64) -> Self {
        Self {
            deadline: now_ms().checked_add(ms).unwrap(),
        }
    }
    pub fn after_secs(seconds: u64) -> Self {
        Self::after_millis(seconds.checked_mul(1000).unwrap())
    }
}
impl Future for Timer {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> Poll<()> {
        if now_ms() >= self.deadline {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
#[derive(Debug)]
pub struct Timeout;
pub async fn with_timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, Timeout> {
    let mut timer = std::pin::pin!(Timer::after_millis(duration.0));
    let mut future = std::pin::pin!(future);
    poll_fn(|cx| {
        if let Poll::Ready(value) = future.as_mut().poll(cx) {
            return Poll::Ready(Ok(value));
        }
        if timer.as_mut().poll(cx).is_ready() {
            Poll::Ready(Err(Timeout))
        } else {
            Poll::Pending
        }
    })
    .await
}
