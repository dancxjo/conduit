use super::{DebounceOperation, TimeoutOperation};
use conduit_kernel::{
    HostOperationDisposition, HostOperationOutcome, OperationAction, OperationInput, PortId,
    RequestId, ValueRef,
};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn completion(request: u32, disposition: HostOperationDisposition) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition,
            output: None,
            failure: None,
        },
    }
}

fn debounce() -> DebounceOperation {
    DebounceOperation {
        durations: vec![value(10, 8), value(11, 8), value(12, 8)],
        next_request: 0,
        maximum_values: 3,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        candidate: None,
        released: None,
        terminal_releases: Vec::with_capacity(8),
        retain_resumed: false,
        closing: false,
        complete_after_emit: false,
    }
}

fn timeout() -> TimeoutOperation {
    TimeoutOperation {
        durations: vec![value(20, 8), value(21, 8), value(22, 8)],
        false_values: vec![value(30, 1), value(31, 1), value(32, 1)],
        true_values: vec![value(40, 1), value(41, 1)],
        next_request: 0,
        next_false: 0,
        next_true: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        terminal_releases: Vec::with_capacity(6),
        timed_out: false,
        closing: false,
        arm_after_emit: false,
    }
}

#[test]
fn debounce_burst_resets_exact_request_and_flushes_pending_value_on_close() {
    let mut operation = debounce();
    assert_eq!(operation.start(), OperationAction::Await);
    let first = value(1, 1);
    let last = value(2, 1);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: first
        }),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            ..
        }
    ));
    assert!(operation.retains_resumed_value());
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: last
        }),
        OperationAction::Await
    );
    assert_eq!(operation.take_released_value(), Some(first));
    assert_eq!(
        operation.take_host_operation_cancellation(),
        Some(RequestId(1))
    );
    assert!(matches!(
        operation.resume(completion(1, HostOperationDisposition::Cancelled)),
        OperationAction::RequestHostOperation {
            request: RequestId(2),
            ..
        }
    ));
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.take_host_operation_cancellation(),
        Some(RequestId(2))
    );
    assert_eq!(operation.take_released_value(), Some(value(12, 8)));
    assert_eq!(
        operation.resume(completion(2, HostOperationDisposition::Cancelled)),
        OperationAction::Emit {
            port: PortId(0),
            value: last
        }
    );
    assert_eq!(operation.advance(), OperationAction::Complete);
}

#[test]
fn timeout_distinguishes_expiry_recovery_reset_and_terminal_cancellation() {
    let mut operation = timeout();
    assert_eq!(
        operation.start(),
        OperationAction::Emit {
            port: PortId(0),
            value: value(30, 1)
        }
    );
    assert!(matches!(
        operation.advance(),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            ..
        }
    ));
    assert_eq!(
        operation.resume(completion(1, HostOperationDisposition::Completed)),
        OperationAction::Emit {
            port: PortId(0),
            value: value(40, 1)
        }
    );
    assert_eq!(operation.advance(), OperationAction::Await);
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: value(50, 8)
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: value(31, 1)
        }
    );
    assert!(matches!(
        operation.advance(),
        OperationAction::RequestHostOperation {
            request: RequestId(2),
            ..
        }
    ));
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.take_host_operation_cancellation(),
        Some(RequestId(2))
    );
    assert_eq!(
        operation.resume(completion(2, HostOperationDisposition::Cancelled)),
        OperationAction::Complete
    );
    let mut released = Vec::new();
    while let Some(value) = operation.take_released_value() {
        released.push(value);
    }
    released.sort_by_key(|value| value.slot);
    assert_eq!(released, vec![value(22, 8), value(32, 1), value(41, 1)]);
}
