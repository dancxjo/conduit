use super::*;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostCallOutcome, ValueRef,
};

fn value(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: SCALAR_ENCODED_LEN as u32,
    }
}

fn operation() -> MathScalarOperation {
    MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: SCALAR_ENCODED_LEN as u32,
    }
}

#[test]
fn quantity_completion_checks_output_bound_and_preserves_failure_detail() {
    use conduit_kernel::{Failure, FailureCode};
    let mut active = MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
    };
    let mut io = StepIo::test_frame([Some(value(1))], [false], [Some(32)], None, 8);
    assert_eq!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let failure = Failure {
        code: FailureCode::InvalidInput,
        detail: 4,
    };
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(32)],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        )),
        8,
    );
    assert_eq!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(failure)
    );

    let mut active = MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
    };
    let mut io = StepIo::test_frame([Some(value(1))], [false], [Some(32)], None, 8);
    assert_eq!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(32)],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        )),
        8,
    );
    assert!(matches!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(_)
    ));
}

#[test]
fn transform_vectors_match_the_portable_no_std_semantics() {
    let one = Scalar::ONE;
    assert_eq!(
        MathTransform::Clamp {
            minimum: Scalar::from_raw_microunits(-Scalar::SCALE),
            maximum: one,
        }
        .apply(Scalar::MAX),
        Ok(one)
    );
    assert_eq!(
        MathTransform::Scale {
            gain: Scalar::from_raw_microunits(2_000_000),
        }
        .apply(Scalar::MAX),
        Err(conduit_semantic_catalog::MathScalarError::Overflow)
    );
    assert_eq!(
        MathTransform::Deadband {
            radius: Scalar::from_raw_microunits(50_000),
        }
        .apply(Scalar::from_raw_microunits(-50_000)),
        Ok(Scalar::ZERO)
    );
}

#[test]
fn operation_requires_one_exact_completion_and_closure_is_terminal() {
    let mut active = operation();
    let mut io = StepIo::test_frame([Some(value(1))], [false], [Some(32)], None, 8);
    assert_eq!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(SCALAR_ENCODED_LEN as u32)],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        )),
        8,
    );
    assert_eq!(
        active.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(value(2)));

    let mut closed = operation();
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        closed.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn cancellation_clears_pending_transform_without_inventing_output() {
    let mut operation = operation();
    let mut io = StepIo::test_frame([Some(value(1))], [false], [Some(32)], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    StepOperation::<1>::cancel(&mut operation);
    assert!(operation.pending.is_none());
    assert!(operation.completed);
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(32)],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        )),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(!io.test_host_completion_consumed());
}
