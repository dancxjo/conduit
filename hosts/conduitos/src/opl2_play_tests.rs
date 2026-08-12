use super::*;
use crate::machine::BaseError;

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
fn ordinary_plan_runs_chord_octaves_and_exact_nine_voice_saturation() {
    let prepared = prepared();
    let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
    let mut base = RecordedOpl2::default();
    let report = run(&mut execution, &mut base).unwrap();
    assert_eq!(report.events, 24);
    assert_eq!(report.peak_voices, 9);
    assert_eq!(report.reset_writes, 245);
    assert_eq!(report.patch_writes, 99);
    assert_eq!(report.event_writes, 39);
    assert_eq!(report.quiesce_writes, 9);
    assert_eq!(report.final_active_voices, 0);
    assert!(report.completed);
    assert_eq!(base.active, [false; 9]);
}

#[test]
fn tenth_voice_refuses_and_quiesces_every_owned_channel() {
    let prepared = prepared();
    let mut values = reviewed_values();
    values[21] = note(19, 587_330, Gate::On, 21);
    values[22] = note(10, 220_000, Gate::Off, 22);
    values[23] = note(19, 587_330, Gate::Off, 23);
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
    assert_eq!(cancel(&mut execution, &mut base), Ok(9));
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
