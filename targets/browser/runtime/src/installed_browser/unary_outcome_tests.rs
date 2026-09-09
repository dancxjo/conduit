use super::*;
use conduit_kernel::HostOperationOutcome;

fn pending() -> UnaryOperation {
    UnaryOperation {
        maximum_input_bytes: 4096,
        next_request: 3,
        pending: Some(RequestId(3)),
    }
}

#[test]
fn unary_failure_preserves_exact_matched_host_failure_and_cancellation() {
    let failure = Failure {
        code: FailureCode::InvalidInput,
        detail: 123,
    };
    let mut operation = pending();
    assert!(
        matches!(operation.resume(OperationInput::HostOperationCompleted {
        request: RequestId(3), outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Failed, output: None, failure: Some(failure),
        },
    }), OperationAction::Fail(found) if found == failure)
    );
    assert!(operation.pending.is_none());
    let mut operation = pending();
    assert!(matches!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(3),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Cancelled,
                output: None,
                failure: None,
            },
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 0
        })
    ));
}

#[test]
fn unary_stale_completion_and_malformed_failure_do_not_claim_the_supplied_failure() {
    for (request, failure) in [
        (
            RequestId(4),
            Some(Failure {
                code: FailureCode::InvalidInput,
                detail: 123,
            }),
        ),
        (RequestId(3), None),
    ] {
        let mut operation = pending();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request,
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure,
                },
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2
            })
        ));
    }
}

#[test]
fn unary_transform_reuses_one_host_slot_for_later_open_flow_values() {
    let mut operation = UnaryOperation {
        maximum_input_bytes: 16,
        next_request: 0,
        pending: None,
    };
    assert_eq!(operation.start(), OperationAction::Await);
    for slot in 1..=3 {
        let input = ValueRef {
            slot,
            generation: 1,
            byte_len: 8,
        };
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: input,
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(found),
                ..
            } if found == u32::from(slot - 1)
        ));
        let output = ValueRef {
            slot: slot + 10,
            generation: 1,
            byte_len: 8,
        };
        assert_eq!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(u32::from(slot - 1)),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(output, 16).unwrap()),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: output,
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
    }
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
}
