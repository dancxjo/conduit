use super::*;
use crate::machine::{BaseError, FixedTimerSlots, KernelInterest, TimerBase, TimerToken};

#[derive(Default)]
struct Timer {
    slots: FixedTimerSlots<1>,
    armed: Option<TimerToken>,
    wakes: u32,
}

impl TimerBase for Timer {
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        let token = self.slots.arm(interest)?;
        self.armed = Some(token);
        Ok(token)
    }

    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        self.armed = None;
        self.slots.cancel(token)
    }

    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        let Some(token) = self.armed.take() else {
            return Ok(None);
        };
        self.wakes += 1;
        self.slots.wake(token).map(Some)
    }

    fn wake_count(&self) -> u32 {
        self.wakes
    }
}

#[test]
fn standing_timer_stage_presents_zero_then_one_and_stops_with_next_wait_pending() {
    let (identities, offer) = fixture();
    let mut product = TourProduct::canonical(10);
    let mut clock = Clock::default();
    let mut timer = Timer::default();
    let mut serial = Serial::default();
    let mut interrupts = Interrupts::default();
    let mut idle = Idle::default();
    for revision in [10, 11] {
        product
            .accept_with_timer(
                &event(revision, NEXT_CHAPTER_ACTION_ID),
                &identities,
                &offer,
                "build",
                &mut clock,
                &mut timer,
                &mut serial,
                &mut interrupts,
                &mut idle,
            )
            .unwrap();
    }
    let update = product
        .accept_with_timer(
            &event(12, RUN_ACTION_ID),
            &identities,
            &offer,
            "build",
            &mut clock,
            &mut timer,
            &mut serial,
            &mut interrupts,
            &mut idle,
        )
        .unwrap();
    let evidence = update.play.unwrap();
    assert_eq!(evidence.specimen_id, "canonical-form:count-over-time");
    assert_eq!(evidence.result, "1");
    assert_eq!(
        evidence.terminal,
        conduit_tour_model::TourRunTerminal::Stopped
    );
    assert_eq!(evidence.run.logical_operations, 3);
    assert_eq!(evidence.run.timer_irq_wakes, 1);
    assert_eq!(evidence.run.pending_host_operations, 1);
    assert!(evidence.run.timer_pending_during_text_progress);
    assert_eq!(serial.0, [b"0".as_slice(), b"1".as_slice()]);
    let proof = product.controller().last_run().unwrap();
    assert_eq!(proof.terminal, conduit_tour_model::TourRunTerminal::Stopped);
}
