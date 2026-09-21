#![cfg(feature = "kernel-operation")]
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::PatternComparisonOperation;

fn value(byte_len: u32) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len,
    }
}

#[test]
fn host_input_bound_is_exact_and_oversize_refuses() {
    for bound in [4096, 65536] {
        let mut operation = PatternComparisonOperation::new(bound);
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(bound)
            }),
            OperationAction::RequestHostCall {
                request: RequestId(0),
                operation: HostCallId(0),
                input: BoundedValueRef::new(value(bound), bound).unwrap()
            }
        );
        let mut operation = PatternComparisonOperation::new(bound);
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(bound + 1)
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 254
            })
        );
    }
}

#[test]
fn either_port_order_emits_once_then_requires_both_closures() {
    for ports in [[0, 1], [1, 0]] {
        let mut operation = PatternComparisonOperation::new(4096);
        assert_eq!(operation.start(), OperationAction::Await);
        for (index, port) in ports.into_iter().enumerate() {
            let request = RequestId(index as u32);
            assert!(
                matches!(operation.resume(OperationInput::Value { port: PortId(port), value: value(10) }),
                OperationAction::RequestHostCall { request: actual, operation: HostCallId(actual_port), .. }
                if actual == request && actual_port == port)
            );
            let output = (index == 1).then(|| BoundedValueRef::new(value(20), 4096).unwrap());
            let action = operation.resume(OperationInput::HostCallCompleted {
                request,
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output,
                    failure: None,
                },
            });
            assert_eq!(
                action,
                if index == 0 {
                    OperationAction::Await
                } else {
                    OperationAction::Emit {
                        port: PortId(0),
                        value: value(20),
                    }
                }
            );
        }
        assert_eq!(operation.advance(), OperationAction::Await);
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(10),
            }),
            OperationAction::RequestHostCall {
                request: RequestId(2),
                operation: HostCallId(0),
                input: BoundedValueRef::new(value(10), 4096).unwrap(),
            }
        );
        assert_eq!(
            operation.resume(OperationInput::HostCallCompleted {
                request: RequestId(2),
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value(20), 4096).unwrap()),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: value(20),
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Await
        );
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(1) }),
            OperationAction::Complete
        );
    }
}

#[test]
fn stale_completion_and_cancelled_work_do_not_emit() {
    let mut operation = PatternComparisonOperation::new(4096);
    operation.resume(OperationInput::Value {
        port: PortId(0),
        value: value(10),
    });
    operation.cancel();
    let action = operation.resume(OperationInput::HostCallCompleted {
        request: RequestId(0),
        outcome: HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(value(20), 4096).unwrap()),
            failure: None,
        },
    });
    assert_eq!(
        action,
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 251
        })
    );
}
