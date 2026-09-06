use super::*;
use conduit_kernel::HostOperationOutcome;

fn pending() -> UnaryOperation {
    UnaryOperation {
        maximum_input_bytes: 4096,
        maximum_values: 4,
        next: 0,
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
