use super::*;
use conduit_kernel::{BoundedValueRef, HostOperationOutcome, OperationAction, OperationInput};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn complete(request: u32, output: Option<BoundedValueRef>) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output,
            failure: None,
        },
    }
}

fn initialized_operation() -> LeniaStepOperation {
    let seed = conduit_alife::orbium_seed(32, 32, 1)
        .unwrap()
        .encode()
        .unwrap();
    let mut operation = LeniaStepOperation::new();
    assert!(matches!(
        operation.resume_value(PortId(0), value(1, seed.len() as u32), &seed),
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            ..
        }
    ));
    assert_eq!(operation.resume(complete(0, None)), OperationAction::Await);
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    operation
}

#[test]
fn value_closure_then_ordered_tick_is_the_only_accepted_lifecycle() {
    let mut operation = initialized_operation();
    let tick = super::super::contract::encode_tick(0);
    let tick_ref = value(2, tick.len() as u32);
    assert!(matches!(
        operation.resume_value(PortId(1), tick_ref, &tick),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: HostOperationId(1),
            ..
        }
    ));
    let field = BoundedValueRef::new(
        value(3, LENIA_MAXIMUM_FIELD_BYTES),
        LENIA_MAXIMUM_FIELD_BYTES,
    )
    .unwrap();
    assert_eq!(
        operation.resume(complete(1, Some(field))),
        OperationAction::Emit {
            port: PortId(0),
            value: field.value,
        }
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Complete
    );
}

#[test]
fn reordered_tick_and_completion_after_cancel_remain_machine_readable_failures() {
    let mut reordered = initialized_operation();
    let tick = super::super::contract::encode_tick(1);
    assert_eq!(
        reordered.resume_value(PortId(1), value(2, tick.len() as u32), &tick),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 183,
        })
    );

    let mut cancelled = initialized_operation();
    let tick = super::super::contract::encode_tick(0);
    cancelled.resume_value(PortId(1), value(2, tick.len() as u32), &tick);
    cancelled.cancel();
    assert_eq!(
        cancelled.resume(complete(1, None)),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 188,
        })
    );
}
