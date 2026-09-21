use super::*;
use conduit_kernel::HostCallOutcome;

fn pending() -> UnaryBack {
    UnaryBack {
        maximum_input_bytes: 4096,
        next_request: 3,
        pending: Some(RequestId(3)),
    }
}

fn completion(
    operation: &mut UnaryBack,
    request: RequestId,
    outcome: HostCallOutcome,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame([None], [false], [Some(4096)], Some((request, outcome)), 4);
    let result = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (result, io)
}

#[test]
fn unary_failure_preserves_exact_matched_host_failure_and_cancellation() {
    let failure = Failure {
        code: FailureCode::InvalidInput,
        detail: 123,
    };
    let mut operation = pending();
    assert_eq!(
        completion(
            &mut operation,
            RequestId(3),
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        )
        .0,
        StepOutcome::Fail(failure)
    );
    assert!(operation.pending.is_none());
    let mut operation = pending();
    assert_eq!(
        completion(
            &mut operation,
            RequestId(3),
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .0,
        StepOutcome::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 0,
        })
    );
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
        assert_eq!(
            completion(
                &mut operation,
                request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure,
                },
            )
            .0,
            fail(2)
        );
    }
}

#[test]
fn unary_transform_reuses_one_host_slot_for_later_open_flow_values() {
    let mut operation = UnaryBack {
        maximum_input_bytes: 16,
        next_request: 0,
        pending: None,
    };
    for slot in 1..=3 {
        let input = ValueRef {
            slot,
            generation: 1,
            byte_len: 8,
        };
        let mut io = StepIo::test_frame([Some(input)], [false], [Some(16)], None, 4);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(u32::from(slot - 1)))
        );
        let output = ValueRef {
            slot: slot + 10,
            generation: 1,
            byte_len: 8,
        };
        let (result, io) = completion(
            &mut operation,
            RequestId(u32::from(slot - 1)),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 16).unwrap()),
                failure: None,
            },
        );
        assert_eq!(result, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), Some(output));
    }
    let mut io = StepIo::test_frame([None], [true], [None], None, 4);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}
