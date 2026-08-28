use super::*;
use crate::machine::BaseError;
use conduit_core::{CancellationReason, TerminalDisposition};

#[derive(Default)]
struct RecordedOpl2 {
    active: [bool; 9],
    writes: u16,
    fail_quiesce: bool,
}

impl Opl2Base for RecordedOpl2 {
    fn reset(&mut self) -> Result<u16, BaseError> {
        self.active.fill(false);
        self.writes += 245;
        Ok(245)
    }

    fn configure_fixed_patch(&mut self, channel: u8) -> Result<u16, BaseError> {
        if channel >= 9 {
            return Err(BaseError::OutOfRange);
        }
        self.writes += 11;
        Ok(11)
    }

    fn key_on(&mut self, channel: u8, requested: u64) -> Result<Opl2Pitch, BaseError> {
        let active = self
            .active
            .get_mut(usize::from(channel))
            .ok_or(BaseError::OutOfRange)?;
        if *active {
            return Err(BaseError::SlotFull);
        }
        *active = true;
        self.writes += 2;
        Ok(Opl2Pitch {
            requested_millihertz: requested,
            realized_millihertz: requested,
            f_number: 512,
            block: 4,
        })
    }

    fn key_off(&mut self, channel: u8) -> Result<(), BaseError> {
        let active = self
            .active
            .get_mut(usize::from(channel))
            .ok_or(BaseError::OutOfRange)?;
        *active = false;
        self.writes += 1;
        Ok(())
    }

    fn quiesce(&mut self) -> Result<u16, BaseError> {
        if self.fail_quiesce {
            return Err(BaseError::Unavailable);
        }
        self.active.fill(false);
        self.writes += 9;
        Ok(9)
    }
}

fn prepared() -> crate::opl2_plan::PreparedOpl2Play {
    let (identities, fixed, opl2) = crate::opl2_plan::tests::fixture();
    crate::opl2_plan::prepare(&identities, &fixed, opl2, "build").unwrap()
}

#[test]
fn ordinary_plan_runs_chord_and_exact_nine_voice_saturation() {
    let prepared = prepared();
    let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
    let mut base = RecordedOpl2::default();
    let report = run_with_evidence(&prepared, &mut execution, &mut base).unwrap();
    assert_eq!(report.play.events, 24);
    assert_eq!(report.play.peak_voices, 9);
    assert_eq!(report.play.reset_writes, 245);
    assert_eq!(report.play.patch_writes, 99);
    assert_eq!(report.play.event_writes, 36);
    assert_eq!(report.play.quiesce_writes, 9);
    assert_eq!(report.play.final_active_voices, 0);
    assert!(report.play.completed);
    assert_eq!(report.evidence.selected.plan_id, prepared.plan.plan_id);
    assert_eq!(
        report.evidence.selected.host_id,
        prepared.active_play.host_id
    );
    assert_eq!(
        report.evidence.selected.boot_id,
        prepared.active_play.boot_id
    );
    assert_eq!(report.evidence.trace.events.len(), 24);
    assert_eq!(
        report.evidence.trace.terminal,
        TerminalDisposition::Completed
    );
    assert!(
        report
            .evidence
            .trace
            .events
            .iter()
            .all(|event| event.requested_pitch_millihertz == event.admitted_pitch_millihertz)
    );
    assert_eq!(base.active, [false; 9]);
}

#[test]
fn tenth_voice_refuses_and_quiesces_every_owned_channel() {
    let prepared = prepared();
    let mut values = reviewed_values();
    values[15] = note(19, 587_330, Gate::On, 15);
    let mut execution = prepare_execution(&prepared, values).unwrap();
    let mut base = RecordedOpl2::default();
    assert_eq!(
        run(&mut execution, &mut base),
        Err(PreparationError::KernelRejected)
    );
    assert_eq!(base.active, [false; 9]);
}

#[test]
fn active_cancellation_and_reset_failure_are_distinct_and_silent() {
    let prepared = prepared();
    let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
    let mut base = RecordedOpl2::default();
    let (writes, evidence) = cancel_with_evidence(
        &prepared,
        &mut execution,
        &mut base,
        CancellationReason::OperatorRequested,
    )
    .unwrap();
    assert_eq!(writes, 9);
    assert_eq!(
        evidence.trace.terminal,
        TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        }
    );
    assert!(evidence.trace.events.is_empty());
    assert_eq!(base.active, [false; 9]);

    let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
    let mut failed = RecordedOpl2 {
        fail_quiesce: true,
        ..RecordedOpl2::default()
    };
    assert_eq!(
        cancel(&mut execution, &mut failed),
        Err(PreparationError::KernelRejected)
    );
}

#[test]
fn unsupported_velocity_refuses_without_claiming_expression() {
    let prepared = prepared();
    let mut values = reviewed_values();
    values[0].velocity = 32_768;
    let mut execution = prepare_execution(&prepared, values).unwrap();
    let mut base = RecordedOpl2::default();
    assert_eq!(
        run(&mut execution, &mut base),
        Err(PreparationError::KernelRejected)
    );
    assert_eq!(base.active, [false; 9]);
}
