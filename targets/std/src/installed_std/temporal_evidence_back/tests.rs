use super::*;
use conduit_ai::{
    ClockBasis, TemporalEvidenceFlowSelection, TemporalEvidenceIdentity, TemporalEvidenceOutcome,
    TemporalEvidenceWindow, TemporalEvidenceWindowCandidates, TemporalReference,
    TemporalRetrievalIntent,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn back() -> TemporalEvidenceBack {
    TemporalEvidenceBack {
        selector: BoundedTemporalEvidenceFlow::prepare(),
        staged: false,
        complete: false,
    }
}
fn value(bytes: &[u8], slot: u16) -> conduit_kernel::ValueRef {
    conduit_kernel::ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}
fn window(identity: &str) -> Vec<u8> {
    TemporalEvidenceWindow::new(
        TemporalEvidenceWindowCandidates::new(BoundedSequence::new()).unwrap(),
        false,
        TemporalEvidenceIdentity::new(identity.into()).unwrap(),
        TemporalReference {
            reference_at: 100,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
        },
    )
    .unwrap()
    .encode()
    .unwrap()
}
#[test]
fn pressure_precedes_parsing_and_cancellation_discards_work() {
    let mut back = back();
    let bad = b"bad";
    let bytes = StepInputBytes::test_frame([Some(bad.as_slice()); 2], None);
    let mut blocked = StepIo::test_frame(
        [Some(value(bad, 0)), Some(value(bad, 1))],
        [false; 2],
        [None; 2],
        None,
        8,
    );
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    assert!(!blocked.test_consumed(WINDOW));
    assert!(!blocked.test_consumed(INTENT));
    StepBack::<2>::cancel(&mut back);
    let mut ready = StepIo::test_frame(
        [Some(value(bad, 0)), Some(value(bad, 1))],
        [false; 2],
        [Some(MAX as u32), None],
        None,
        8,
    );
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Complete);
    assert!(StepBack::<2>::prepared_output(&back, OUTCOME).is_none());
}
#[test]
fn two_windows_publish_exact_outcomes_atomically_and_then_close() {
    let mut back = back();
    let intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    for identity in ["window/one", "window/two"] {
        let input = window(identity);
        let bytes =
            StepInputBytes::test_frame([Some(input.as_slice()), Some(intent.as_slice())], None);
        let inputs = [Some(value(&input, 0)), Some(value(&intent, 1))];
        let mut small = StepIo::test_frame(inputs, [false; 2], [Some(1), None], None, 8);
        assert_eq!(back.step(&mut small, &bytes), StepOutcome::Await);
        assert!(!small.test_consumed(WINDOW));
        assert!(!small.test_consumed(INTENT));
        let mut ready = StepIo::test_frame(inputs, [false; 2], [Some(MAX as u32), None], None, 8);
        assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
        let output = TemporalEvidenceOutcome::decode(
            StepBack::<2>::prepared_output(&back, OUTCOME).unwrap(),
        )
        .unwrap();
        assert_eq!(output.window_identity().get(), identity);
        assert!(matches!(
            output.selection(),
            TemporalEvidenceFlowSelection::NoHistory
        ));
        assert!(ready.test_consumed(WINDOW));
        assert!(ready.test_consumed(INTENT));
        StepBack::<2>::step_committed(&mut back);
        assert!(StepBack::<2>::prepared_output(&back, OUTCOME).is_none());
    }
    let bytes = StepInputBytes::test_frame([None; 2], None);
    let mut closed =
        StepIo::test_frame([None; 2], [true, false], [Some(MAX as u32), None], None, 8);
    assert_eq!(back.step(&mut closed, &bytes), StepOutcome::Complete);
    assert!(StepBack::<2>::prepared_output(&back, OUTCOME).is_none());
}
