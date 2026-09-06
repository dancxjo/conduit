#![cfg(feature = "kernel-operation")]
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::TemplateStorageOperation;

fn value(bytes: u32) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes,
    }
}
fn input(bytes: u32) -> OperationInput {
    OperationInput::Value {
        port: PortId(0),
        value: value(bytes),
    }
}
fn completed(request: u32) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: Some(BoundedValueRef::new(value(10), 4096).unwrap()),
            failure: None,
        },
    }
}
fn failure(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[test]
fn exact_host_bounds_accept_and_oversize_refuses() {
    for bound in [4096, 65536] {
        let mut operation = TemplateStorageOperation::new(2, bound);
        assert_eq!(
            operation.resume(input(bound)),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                operation: HostOperationId(0),
                input: BoundedValueRef::new(value(bound), bound).unwrap(),
            }
        );
        let mut operation = TemplateStorageOperation::new(2, bound);
        assert_eq!(
            operation.resume(input(bound + 1)),
            failure(FailureCode::InvalidInput, 264)
        );
    }
}

#[test]
fn finite_commands_emit_exactly_then_refuse_excess_and_allow_closure() {
    let mut operation = TemplateStorageOperation::new(2, 4096);
    assert_eq!(operation.start(), OperationAction::Await);
    for request in 0..2 {
        assert!(
            matches!(operation.resume(input(10)), OperationAction::RequestHostOperation {
            request: RequestId(actual), operation: HostOperationId(0), ..
        } if actual == request)
        );
        assert_eq!(
            operation.resume(completed(request)),
            OperationAction::Emit {
                port: PortId(0),
                value: value(10)
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
    }
    assert_eq!(
        operation.resume(input(10)),
        failure(FailureCode::StorageExhausted, 262)
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
}

#[test]
fn cancellation_invalidates_pending_completion_and_host_failures_remain_exact() {
    let mut operation = TemplateStorageOperation::new(2, 4096);
    operation.resume(input(10));
    operation.cancel();
    assert_eq!(
        operation.resume(completed(0)),
        failure(FailureCode::InvalidLifecycle, 261)
    );
    let mut operation = TemplateStorageOperation::new(2, 4096);
    operation.resume(input(10));
    let reason = Failure {
        code: FailureCode::InvalidInput,
        detail: 4,
    };
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(reason),
            }
        }),
        OperationAction::Fail(reason)
    );
}
