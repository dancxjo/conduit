use super::*;
use crate::{machine::BaseError, pc_speaker_plan};

#[derive(Default)]
struct FakeBase {
    active: bool,
    transitions: u32,
}

impl ToneBase for FakeBase {
    fn apply(&mut self, intent: ToneIntent) -> Result<RealizedTone, BaseError> {
        self.active = intent.gate == Gate::On;
        self.transitions += 1;
        Ok(RealizedTone {
            correlation: intent.correlation,
            requested_millihertz: intent.pitch.frequency_millihertz,
            realized_millihertz: if self.active {
                intent.pitch.frequency_millihertz
            } else {
                0
            },
            divisor: if self.active { 1 } else { 0 },
            gate_open: self.active,
        })
    }

    fn silence(&mut self) -> Result<(), BaseError> {
        self.active = false;
        self.transitions += 1;
        Ok(())
    }

    fn transition_count(&self) -> u32 {
        self.transitions
    }
}

#[test]
fn ordinary_plan_drives_ordered_tones_through_host_boundary_and_closes_gate() {
    let (identities, offer) = pc_speaker_plan::tests::fixture();
    let prepared = pc_speaker_plan::prepare(&identities, &offer, "build").unwrap();
    let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
    let mut base = FakeBase::default();
    let report = run(&mut execution, &mut base).unwrap();
    assert_eq!(
        report.realized.map(|tone| tone.gate_open),
        [true, false, true, false]
    );
    assert_eq!(report.transitions, 4);
    assert!(!report.final_gate_open);
    assert!(report.completed);
}

#[test]
fn cancellation_closes_gate_and_rejects_late_completion() {
    let mut kernel = PcSpeakerKernel::prepare(reviewed_values()).unwrap();
    let mut base = FakeBase::default();
    let pending = loop {
        kernel.scheduler.step().unwrap();
        let Some(request) = kernel.next_request() else {
            continue;
        };
        if request.node == SOURCE_NODE {
            kernel.complete(request).unwrap();
            continue;
        }
        let intent = ToneIntent::decode(kernel.host_value(request.input.value).unwrap()).unwrap();
        assert!(base.apply(intent).unwrap().gate_open);
        break request;
    };
    kernel.scheduler.cancel().unwrap();
    base.silence().unwrap();
    assert!(!base.active);
    assert_eq!(kernel.scheduler.step(), Ok(SchedulerStatus::Cancelled));
    assert_eq!(
        kernel.complete(pending),
        Err(SchedulerError::HostOperationCompletionRejected)
    );
}

#[test]
fn repeated_start_stop_cycles_never_leave_the_gate_open() {
    let (identities, offer) = pc_speaker_plan::tests::fixture();
    let prepared = pc_speaker_plan::prepare(&identities, &offer, "build").unwrap();
    let mut base = FakeBase::default();
    for _ in 0..2 {
        let mut execution = prepare_execution(&prepared, reviewed_values()).unwrap();
        let report = run(&mut execution, &mut base).unwrap();
        assert!(!report.final_gate_open);
        assert!(!base.active);
    }
    assert_eq!(base.transitions, 8);
}

#[test]
fn malformed_plan_refuses_before_base_effect() {
    let (identities, offer) = pc_speaker_plan::tests::fixture();
    let mut prepared = pc_speaker_plan::prepare(&identities, &offer, "build").unwrap();
    prepared.active_play.play_sequence = 9;
    assert_eq!(
        prepare_execution(&prepared, reviewed_values()).err(),
        Some(PreparationError::PlanRejected)
    );
}
