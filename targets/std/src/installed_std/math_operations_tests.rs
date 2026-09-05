use super::*;
use conduit_kernel::{HostOperationOutcome, ValueRef};

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
    active.resume(OperationInput::Value {
        port: PortId(0),
        value: value(1),
    });
    let failure = Failure {
        code: FailureCode::InvalidInput,
        detail: 4,
    };
    assert_eq!(
        active.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        }),
        OperationAction::Fail(failure)
    );

    let mut active = MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
    };
    active.resume(OperationInput::Value {
        port: PortId(0),
        value: value(1),
    });
    assert!(matches!(
        active.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        }),
        OperationAction::Fail(_)
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
    assert_eq!(active.start(), OperationAction::Await);
    assert!(matches!(
        active.resume(OperationInput::Value {
            port: PortId(0),
            value: value(1),
        }),
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            ..
        }
    ));
    assert!(matches!(
        active.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: emitted,
        } if emitted == value(2)
    ));

    let mut closed = operation();
    assert_eq!(
        closed.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
}

#[test]
fn cancellation_clears_pending_transform_without_inventing_output() {
    let mut operation = operation();
    operation.resume(OperationInput::Value {
        port: PortId(0),
        value: value(1),
    });
    operation.cancel();
    assert!(operation.pending.is_none());
    assert!(operation.completed);
    assert!(matches!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(value(2), SCALAR_ENCODED_LEN as u32).unwrap()),
                failure: None,
            },
        }),
        OperationAction::Fail(_)
    ));
}
