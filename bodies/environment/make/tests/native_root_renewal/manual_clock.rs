//! CPU-only monotonic clock / executor wake adapter. No MCU timing claim.
//! Sixteen outstanding Embassy timer wakers; overflow is an explicit proof failure.
use core::task::Waker;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};

static NOW: AtomicU64 = AtomicU64::new(0);
static PENDED: AtomicBool = AtomicBool::new(false);
static TIMERS: Mutex<[Option<(u64, Waker)>; 16]> = Mutex::new([const { None }; 16]);
struct ManualClock;
embassy_time_driver::time_driver_impl!(static DRIVER: ManualClock = ManualClock);
impl embassy_time_driver::Driver for ManualClock {
    fn now(&self) -> u64 {
        NOW.load(Ordering::SeqCst)
    }
    fn schedule_wake(&self, at: u64, waker: &Waker) {
        if at <= self.now() {
            waker.wake_by_ref();
            return;
        }
        let mut timers = TIMERS.lock().unwrap();
        if let Some(slot) = timers
            .iter_mut()
            .flatten()
            .find(|(_, old)| old.will_wake(waker))
        {
            slot.0 = slot.0.min(at);
            return;
        }
        *timers
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("bounded manual timer queue") = Some((at, waker.clone()));
    }
}
#[unsafe(export_name = "__pender")]
fn pender(_: *mut ()) {
    PENDED.store(true, Ordering::SeqCst);
}

/// Poll only on the original thread; raw::Executor requires this contract.
/// The leaked executor lives for the complete test, including every task slot.
pub struct Executor {
    inner: &'static embassy_executor::raw::Executor,
    thread: core::marker::PhantomData<std::rc::Rc<()>>,
}
impl Executor {
    pub fn new() -> Self {
        Self {
            inner: Box::leak(Box::new(embassy_executor::raw::Executor::new(
                core::ptr::null_mut(),
            ))),
            thread: core::marker::PhantomData,
        }
    }
    pub fn spawner(&self) -> embassy_executor::Spawner {
        self.inner.spawner()
    }
    pub fn poll_ready(&self) {
        for _ in 0..1_000_000 {
            PENDED.store(false, Ordering::SeqCst);
            // SAFETY: test creates/polls this executor on this single thread;
            // no recursive poll, and the allocation is permanent.
            unsafe { self.inner.poll() };
            if !PENDED.load(Ordering::SeqCst) {
                return;
            }
        }
        panic!("CPU executor failed bounded cooperative progress");
    }
    pub fn advance_millis(&self, millis: u64) {
        assert_eq!(embassy_time_driver::TICK_HZ, 1_000_000);
        let now = NOW
            .load(Ordering::SeqCst)
            .checked_add(millis.checked_mul(1000).unwrap())
            .unwrap();
        NOW.store(now, Ordering::SeqCst);
        let mut due = [const { None }; 16];
        {
            let mut timers = TIMERS.lock().unwrap();
            for (index, slot) in timers.iter_mut().enumerate() {
                if slot.as_ref().is_some_and(|(at, _)| *at <= now) {
                    due[index] = slot.take().map(|(_, wake)| wake);
                }
            }
        }
        for wake in due.into_iter().flatten() {
            wake.wake();
        }
        self.poll_ready();
    }
    pub fn advance_next_timer(&self) -> bool {
        self.poll_ready();
        let mut due = [const { None }; 16];
        {
            let mut timers = TIMERS.lock().unwrap();
            let Some(next) = timers.iter().flatten().map(|(at, _)| *at).min() else {
                return false;
            };
            assert!(next >= NOW.load(Ordering::SeqCst));
            NOW.store(next, Ordering::SeqCst);
            for (index, slot) in timers.iter_mut().enumerate() {
                if slot.as_ref().is_some_and(|(at, _)| *at <= next) {
                    due[index] = slot.take().map(|(_, waker)| waker);
                }
            }
        }
        for waker in due.into_iter().flatten() {
            waker.wake();
        }
        self.poll_ready();
        true
    }
}
