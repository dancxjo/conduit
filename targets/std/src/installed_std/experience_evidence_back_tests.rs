use super::experience_evidence_back::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

fn back() -> ExperienceEvidenceBack {
    ExperienceEvidenceBack::new()
}

#[test]
fn pressure_does_not_consume_or_parse_and_cancel_never_emits_context() {
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 3,
    };
    let bytes = StepInputBytes::test_frame([Some(&b"bad"[..])], None);
    let mut back = back();
    let mut pressured = StepIo::test_frame([Some(value)], [false], [None], None, 2);
    assert_eq!(back.step(&mut pressured, &bytes), StepOutcome::Await);
    assert!(!pressured.test_consumed(PortId(0)));
    assert!(pressured.test_prepared_output().is_none());
    StepBack::<1>::cancel(&mut back);
    let mut ready = StepIo::test_frame([Some(value)], [false], [Some(65_536)], None, 2);
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Complete);
    assert!(!ready.test_consumed(PortId(0)));
    assert!(ready.test_prepared_output().is_none());
}

#[test]
fn malformed_request_is_an_exact_invalid_input_not_a_context() {
    let mut back = back();
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 3,
    };
    let bytes = StepInputBytes::test_frame([Some(&b"bad"[..])], None);
    let mut io = StepIo::test_frame([Some(value)], [false], [Some(65_536)], None, 2);
    assert_eq!(
        back.step(&mut io, &bytes),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 1,
        })
    );
    assert!(!io.test_consumed(PortId(0)));
    assert!(StepBack::<1>::prepared_output(&back, PortId(0)).is_none());
}

#[test]
fn flow_reuses_prepared_storage_waits_for_bytes_and_consumes_closure() {
    use conduit_human::{
        ExperienceEvidenceContext, ExperienceEvidenceIdentities, ExperienceEvidenceIdentity,
        ExperienceEvidenceItems, ExperienceEvidenceRequest, ExperienceEvidenceSelection,
    };
    use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
    let mut back = ExperienceEvidenceBack::for_flow(true);
    for revision in [1, 2] {
        let selection = ExperienceEvidenceSelection::new(
            ExperienceEvidenceIdentities::new(BoundedSequence::new()).unwrap(),
            ExperienceEvidenceIdentity::new(format!("provenance/{revision}")).unwrap(),
            revision,
        )
        .unwrap();
        let request = ExperienceEvidenceRequest::new(
            ExperienceEvidenceItems::new(BoundedSequence::new()).unwrap(),
            selection,
        )
        .unwrap()
        .encode()
        .unwrap();
        let value = ValueRef {
            slot: 0,
            generation: revision as u16,
            byte_len: request.len() as u32,
        };
        let bytes = StepInputBytes::test_frame([Some(request.as_slice())], None);
        // A free item slot with insufficient byte capacity must still wait.
        let mut pressured = StepIo::test_frame([Some(value)], [false], [Some(1)], None, 2);
        assert_eq!(back.step(&mut pressured, &bytes), StepOutcome::Await);
        assert!(!pressured.test_consumed(PortId(0)));
        assert!(pressured.test_prepared_output().is_none());
        let mut ready = StepIo::test_frame([Some(value)], [false], [Some(65_536)], None, 2);
        assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
        let context = ExperienceEvidenceContext::decode(
            StepBack::<1>::prepared_output(&back, PortId(0)).unwrap(),
        )
        .unwrap();
        assert_eq!(*context.selection().revision(), revision);
        assert!(ready.test_consumed(PortId(0)));
        StepBack::<1>::step_committed(&mut back);
        assert!(StepBack::<1>::prepared_output(&back, PortId(0)).is_none());
    }
    let mut closed = StepIo::test_frame([None], [true], [Some(65_536)], None, 2);
    let bytes = StepInputBytes::test_frame([None], None);
    assert_eq!(back.step(&mut closed, &bytes), StepOutcome::Complete);
    assert!(closed.test_consumed_closed(PortId(0)));
    assert!(closed.test_prepared_output().is_none());
}
