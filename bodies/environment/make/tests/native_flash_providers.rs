//! CPU provider integration: actual Timer/gate/mailbox adapters, mock pulse only.
#![allow(dead_code)]
#[path = "native_flash_providers/manual_clock.rs"]
mod manual_clock;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_flash_jobs.rs"]
mod native_flash_jobs;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_flash_wait.rs"]
mod native_flash_wait;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_flash_window.rs"]
mod native_flash_window;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_pulse_fence.rs"]
mod native_pulse_fence;
use conduit_pulse_capture::{
    owner::PulseReadProvider, CheckedPulseRead, PulseReadCapture, PulseReadRefusal,
};
use core::{
    future::Future,
    pin::{pin, Pin},
    task::Poll,
};
use native_flash_window::Gate;
use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
static DONE: AtomicBool = AtomicBool::new(false);
struct Job(u32);
impl native_flash_jobs::Job for Job {
    fn step(&self) -> u32 {
        self.0
    }
}
async fn once<F: Future>(f: Pin<&mut F>) -> Poll<F::Output> {
    let mut f = f;
    core::future::poll_fn(|cx| Poll::Ready(f.as_mut().poll(cx))).await
}
fn checked() -> CheckedPulseRead {
    use conduit_pulse_capture::{PulseReadLimits as L, PulseReadRequest as R};
    CheckedPulseRead::admit(
        R {
            hold_low_us: 1000,
            skip_high_pulses: 0,
            sample_after_rise_us: 10,
            octets: 1,
            capture_timeout_us: 1000,
        },
        L {
            maximum_hold_low_us: 2000,
            maximum_skipped_pulses: 0,
            minimum_sample_offset_us: 1,
            maximum_sample_offset_us: 100,
            maximum_octets: 1,
            maximum_capture_timeout_us: 2000,
        },
    )
    .unwrap()
}
struct Pulse {
    gate: &'static Gate,
    ready: Rc<Cell<bool>>,
    released: Rc<Cell<u32>>,
    refuse: bool,
}
struct Release {
    gate: &'static Gate,
    released: Rc<Cell<u32>>,
}
impl Drop for Release {
    fn drop(&mut self) {
        let now = embassy_time::Instant::now().as_millis();
        assert!(
            matches!(
                self.gate.begin_pulse(now, now + 1),
                Err(native_flash_window::Refusal::Overlap)
            ),
            "inner GPIO release precedes outer pulse guard retirement"
        );
        self.released.set(self.released.get() + 1);
    }
}
impl PulseReadProvider for Pulse {
    async fn read(&mut self, _: CheckedPulseRead) -> Result<PulseReadCapture, PulseReadRefusal> {
        let _release = Release {
            gate: self.gate,
            released: self.released.clone(),
        };
        core::future::poll_fn(|_| {
            if self.ready.get() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        if self.refuse {
            Err(PulseReadRefusal::DeadlineExtent)
        } else {
            Ok(PulseReadCapture::from_complete_octets(
                conduit_pulse_capture::PulseReadDisposition::Complete,
                &[0],
            )
            .unwrap())
        }
    }
}
#[embassy_executor::task]
async fn proof() {
    let gate = Box::leak(Box::new(Gate::new(100, 20, 10).unwrap()));
    let mut mailbox = native_flash_jobs::Mailbox::<Job, (), ()>::new();
    let (mut root, executor) = mailbox.split().unwrap();
    let mut lane = native_flash_wait::WaitLane::new(gate, executor);
    let calls = Cell::new(0);
    {
        let mut waiting = pin!(lane.wait(30, |j, _| {
            assert!(
                manual_clock::next_deadline().is_some(),
                "real timer registered before job"
            );
            calls.set(calls.get() + 1);
            j.0 += 1;
            Ok(())
        }));
        assert!(once(waiting.as_mut()).await.is_pending());
        let deadline = manual_clock::next_deadline().unwrap();
        let wakes = manual_clock::wake_count();
        let id = root.submit(Job(0), 5).ok().unwrap(); // actual task waker registered by WaitLane
        assert!(manual_clock::wake_count() > wakes);
        assert!(once(waiting.as_mut()).await.is_pending());
        assert_eq!(calls.get(), 1);
        assert_eq!(root.take(id).unwrap().owner.0, 1);
        assert_eq!(manual_clock::next_deadline(), Some(deadline));
        assert!(once(waiting.as_mut()).await.is_pending());
        assert_eq!(calls.get(), 1);
        // A second actual pulse fences a queued distinct operation, retaining owner.
        let now = embassy_time::Instant::now().as_millis();
        let pulse = gate.begin_pulse(now, now + 2).unwrap();
        let id = root.submit(Job(7), 5).ok().unwrap();
        assert!(once(waiting.as_mut()).await.is_pending());
        assert_eq!(calls.get(), 1);
        assert_eq!(root.cancel(id).unwrap().0, 7);
        drop(pulse);
        embassy_time::Timer::after_millis(30).await;
        assert!(matches!(once(waiting.as_mut()).await, Poll::Ready(Ok(()))));
    }
    // Cancellation drops the actual wait guard; next owner can enter immediately.
    {
        let mut waiting = pin!(lane.wait(20, |_, _| panic!("empty mailbox")));
        assert!(once(waiting.as_mut()).await.is_pending());
    }
    let now = embassy_time::Instant::now().as_millis();
    drop(gate.begin_wait(now, now + 10).unwrap());
    for duration in [0, 1] {
        let id = root.submit(Job(9), 5).ok().unwrap();
        {
            let mut waiting = pin!(lane.wait(duration, |_, _| panic!("insufficient real wait")));
            let _ = once(waiting.as_mut()).await;
        }
        assert_eq!(root.cancel(id).unwrap().0, 9);
    }
    for (cancel, refuse) in [(true, false), (false, true), (false, false)] {
        let ready = Rc::new(Cell::new(false));
        let released = Rc::new(Cell::new(0));
        let mut pulse = native_pulse_fence::GatedPulse::new(
            Pulse {
                gate,
                ready: ready.clone(),
                released: released.clone(),
                refuse,
            },
            gate,
        );
        {
            let mut capture = pin!(pulse.read(checked()));
            assert!(once(capture.as_mut()).await.is_pending());
            let now = embassy_time::Instant::now().as_millis();
            let wait = gate.begin_wait(now, now + 10).unwrap();
            assert!(matches!(
                wait.try_flash(now, 1),
                Err(native_flash_window::Refusal::Overlap)
            ));
            if !cancel {
                ready.set(true);
                let result = once(capture.as_mut()).await;
                assert!(matches!(result,Poll::Ready(r) if r.is_err()==refuse));
            }
        }
        assert_eq!(released.get(), 1);
        let now = embassy_time::Instant::now().as_millis();
        let wait = gate.begin_wait(now, now + 10).unwrap();
        assert!(wait.try_flash(now, 1).is_ok());
    }
    // Gate refusal occurs before entering the fake provider.
    let now = embassy_time::Instant::now().as_millis();
    let existing = gate.begin_pulse(now, now + 2).unwrap();
    let released = Rc::new(Cell::new(0));
    let mut pulse = native_pulse_fence::GatedPulse::new(
        Pulse {
            gate,
            ready: Rc::new(Cell::new(true)),
            released: released.clone(),
            refuse: false,
        },
        gate,
    );
    assert!(matches!(
        pulse.read(checked()).await,
        Err(PulseReadRefusal::DeadlineExtent)
    ));
    assert_eq!(released.get(), 0);
    drop(existing);
    DONE.store(true, Ordering::SeqCst);
}
#[test]
fn actual_provider_timer_and_pulse_future_custody() {
    let executor = manual_clock::Executor::new();
    executor.spawner().spawn(proof().unwrap());
    executor.poll_ready();
    for _ in 0..8 {
        if DONE.load(Ordering::SeqCst) {
            break;
        }
        assert!(executor.advance_next_timer());
    }
    assert!(DONE.load(Ordering::SeqCst));
}
