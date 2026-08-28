use super::{DelayOperation, ThrottleOperation};
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

fn completed(request: u32) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: None,
            failure: None,
        },
    }
}

fn cancelled(request: u32) -> OperationInput {
    OperationInput::HostOperationCompleted {
        request: RequestId(request),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Cancelled,
            output: None,
            failure: None,
        },
    }
}

#[test]
fn delay_retains_finite_values_and_drains_them_in_order_after_close() {
    let mut operation = DelayOperation {
        durations: vec![value(10, 8), value(11, 8)],
        values: Vec::with_capacity(2),
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        next_value: 0,
        maximum_values: 2,
        pending: None,
        accepted_values: 0,
        retain_resumed: false,
        closing: false,
        continue_after_emit: false,
    };
    let first = value(1, 1);
    let second = value(2, 1);
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
            value: second
        }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(completed(1)),
        OperationAction::Emit {
            port: PortId(0),
            value: first
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
        operation.resume(completed(2)),
        OperationAction::Emit {
            port: PortId(0),
            value: second
        }
    );
    assert_eq!(operation.advance(), OperationAction::Complete);
}

#[test]
fn leading_throttle_drops_during_interval_and_cancels_exact_timer_on_close() {
    let mut operation = ThrottleOperation {
        durations: vec![value(10, 8), value(11, 8)],
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        arm_after_emit: false,
        closing: false,
    };
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: value(1, 1)
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: value(1, 1)
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
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: value(2, 1)
        }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.take_host_operation_cancellation(),
        Some(RequestId(1))
    );
    assert_eq!(operation.resume(cancelled(1)), OperationAction::Complete);
    assert_eq!(operation.take_released_value(), Some(value(11, 8)));
}

#[test]
fn leading_throttle_reopens_only_after_correlated_completion() {
    let mut operation = ThrottleOperation {
        durations: vec![value(10, 8), value(11, 8)],
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        arm_after_emit: false,
        closing: false,
    };
    let first = value(1, 1);
    let second = value(2, 1);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: first
        }),
        OperationAction::Emit { .. }
    ));
    assert!(matches!(
        operation.advance(),
        OperationAction::RequestHostOperation { .. }
    ));
    assert_eq!(operation.resume(completed(1)), OperationAction::Await);
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: second
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: second
        }
    );
}
