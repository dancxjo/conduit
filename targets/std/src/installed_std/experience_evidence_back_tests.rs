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
