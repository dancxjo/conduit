//! Deterministic clock/carrier leaves only; Root supervisor and kernels are real.
use core::{
    future::{poll_fn, Future},
    ops::Add,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    task::Poll,
};
static NOW: AtomicU64 = AtomicU64::new(11);
static CONNECTED: AtomicBool = AtomicBool::new(false);
static DISCONNECT: AtomicBool = AtomicBool::new(false);
static RESETS: AtomicUsize = AtomicUsize::new(0);
pub fn reset() {
    NOW.store(11, Ordering::SeqCst);
    CONNECTED.store(false, Ordering::SeqCst);
    DISCONNECT.store(false, Ordering::SeqCst);
    RESETS.store(0, Ordering::SeqCst);
}
pub fn at(now: u64) {
    NOW.store(now, Ordering::SeqCst)
}
pub fn connect() {
    CONNECTED.store(true, Ordering::SeqCst)
}
pub fn disconnect() {
    DISCONNECT.store(true, Ordering::SeqCst)
}
pub fn resets() -> usize {
    RESETS.load(Ordering::SeqCst)
}
#[derive(Clone, Copy)]
pub struct Instant(u64);
pub struct Duration(u64);
impl Instant {
    pub fn now() -> Self {
        Self(NOW.load(Ordering::SeqCst))
    }
    pub fn as_millis(self) -> u64 {
        self.0
    }
}
impl Duration {
    pub fn from_millis(n: u64) -> Self {
        Self(n)
    }
}
impl Add<Duration> for Instant {
    type Output = Self;
    fn add(self, n: Duration) -> Self {
        Self(self.0 + n.0)
    }
}
pub struct Timer;
impl Timer {
    pub async fn at(at: Instant) {
        poll_fn(|_| {
            if Instant::now().0 >= at.0 {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
    pub async fn after_millis(n: u64) {
        Self::at(Instant::now() + Duration(n)).await
    }
}
pub struct Link;
impl Link {
    pub async fn connected(&mut self) {
        poll_fn(|_| {
            if CONNECTED.load(Ordering::SeqCst) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
    pub async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
        poll_fn(|_| {
            if DISCONNECT.swap(false, Ordering::SeqCst) {
                Poll::Ready(Err(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
    pub async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        Ok(())
    }
    pub fn reset(&mut self) {
        CONNECTED.store(false, Ordering::SeqCst);
        RESETS.fetch_add(1, Ordering::SeqCst);
    }
}
pub fn poll_once(future: core::pin::Pin<&mut impl Future>) {
    let waker = core::task::Waker::noop();
    let mut context = core::task::Context::from_waker(waker);
    assert!(future.poll(&mut context).is_pending());
}
