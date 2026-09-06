#![cfg(feature = "kernel-operation")]
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::StructuredSelectorOperation;
fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 4,
    }
}
fn completion(request: u32, output: bool) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: output.then(|| BoundedValueRef::new(value(), 4096).unwrap()),
            failure: None,
        },
    }
}
#[test]
fn dropped_value_does_not_close_flow_and_next_match_emits() {
    let mut concrete = StructuredSelectorOperation::new(4096);
    let operation: &mut dyn Operation = &mut concrete;
    assert_eq!(operation.start(), OperationAction::Await);
    for request in 0..2 {
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value()
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(request),
                operation: HostOperationId(0),
                input: BoundedValueRef::new(value(), 4096).unwrap(),
            }
        );
        assert_eq!(
            operation.resume(completion(request, request == 1)),
            if request == 0 {
                OperationAction::Await
            } else {
                OperationAction::Emit {
                    port: PortId(0),
                    value: value(),
                }
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
    }
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
}
#[test]
fn cancel_rejects_late_completion() {
    let mut operation = StructuredSelectorOperation::new(4096);
    operation.resume(OperationInput::Value {
        port: PortId(0),
        value: value(),
    });
    operation.cancel();
    assert_eq!(
        operation.resume(completion(0, true)),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 143
        })
    );
}
