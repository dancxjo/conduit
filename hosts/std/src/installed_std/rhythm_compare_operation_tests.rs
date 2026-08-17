use super::*;
use conduit_kernel::{HostOperationOutcome, ProtocolError};

fn value(slot: u16, bytes: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes,
    }
}

fn fresh_operation() -> RhythmCompareOperation {
    RhythmCompareOperation {
        pending: None,
        next_request: 0,
        drain_marker: value(99, 0),
        release_drain_marker: false,
        closed: [false; 2],
        draining_missed: false,
    }
}

fn completed(output: Option<ValueRef>) -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: output.map(|value| BoundedValueRef::new(value, 4096).unwrap()),
        failure: None,
    }
}

#[test]
fn exact_ports_request_admitted_operations_without_retaining_inputs() {
    let mut operation = fresh_operation();
    let performance = value(1, 43);
    assert_eq!(operation.start(), OperationAction::Await);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: performance,
        }),
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(1),
            ..
        }
    ));
    assert!(!operation.retains_resumed_value());
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(1),
            value: value(2, 100),
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: completed(None),
        }),
        OperationAction::Await
    );
    let reference = value(2, 512);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(1),
            value: reference,
        }),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: HostOperationId(2),
            ..
        }
    ));
    assert_eq!(operation.take_released_value(), None);
}

#[test]
fn performance_close_drains_missed_feedback_before_exact_completion() {
    let mut operation = fresh_operation();
    assert!(matches!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            ..
        }
    ));
    let feedback = value(2, 800);
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: completed(Some(feedback)),
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: feedback,
        }
    );
    assert!(matches!(
        operation.advance(),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: HostOperationId(0),
            ..
        }
    ));
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(1),
            outcome: completed(None),
        }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Complete
    );
    assert_eq!(operation.take_released_value(), Some(value(99, 0)));
}

#[test]
fn oversized_value_duplicate_close_and_wrong_completion_fail_closed() {
    let mut operation = fresh_operation();
    let oversized = value(1, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 + 1);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: oversized,
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            ..
        })
    ));
    assert_eq!(
        BoundedValueRef::new(oversized, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
        Err(ProtocolError::HostOperationInputExceeded)
    );
    let mut operation = fresh_operation();
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Await
    );
    assert!(matches!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
}
