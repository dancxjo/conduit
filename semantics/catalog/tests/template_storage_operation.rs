#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::TemplateStorageOperation;

fn value(bytes: u32) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes,
    }
}

fn input(operation: &mut TemplateStorageOperation, bytes: u32) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame([Some(value(bytes))], [false], [Some(4096)], None, 4);
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

fn completion(
    operation: &mut TemplateStorageOperation,
    request: u32,
    outcome: HostCallOutcome,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(4096)],
        Some((RequestId(request), outcome)),
        4,
    );
    let result = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (result, io)
}

fn completed() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(value(10), 4096).unwrap()),
        failure: None,
    }
}

#[test]
fn exact_host_bounds_accept_and_oversize_refuses() {
    for bound in [4096, 65536] {
        let mut operation = TemplateStorageOperation::new(2, bound);
        let (outcome, io) = input(&mut operation, bound);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(
            io.test_host_request(),
            Some((
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(value(bound), bound).unwrap()
            ))
        );
        let mut operation = TemplateStorageOperation::new(2, bound);
        assert_eq!(
            input(&mut operation, bound + 1).0,
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 264
            })
        );
    }
}

#[test]
fn finite_commands_emit_exactly_then_refuse_excess_and_allow_closure() {
    let mut operation = TemplateStorageOperation::new(2, 4096);
    for request in 0..2 {
        let (outcome, io) = input(&mut operation, 10);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(
            io.test_host_request().map(|call| (call.0, call.1)),
            Some((RequestId(request), HostCallId(0)))
        );
        let (outcome, io) = completion(&mut operation, request, completed());
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), Some(value(10)));
    }
    assert_eq!(
        input(&mut operation, 10).0,
        StepOutcome::Fail(Failure {
            code: FailureCode::StorageExhausted,
            detail: 262
        })
    );
    let mut io = StepIo::test_frame([None], [true], [None], None, 4);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn cancellation_invalidates_pending_completion_and_host_failures_remain_exact() {
    let mut operation = TemplateStorageOperation::new(2, 4096);
    input(&mut operation, 10);
    StepBack::<1>::cancel(&mut operation);
    assert_eq!(
        completion(&mut operation, 0, completed()).0,
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 261
        })
    );
    let mut operation = TemplateStorageOperation::new(2, 4096);
    input(&mut operation, 10);
    let reason = Failure {
        code: FailureCode::InvalidInput,
        detail: 4,
    };
    assert_eq!(
        completion(
            &mut operation,
            0,
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(reason),
            },
        )
        .0,
        StepOutcome::Fail(reason)
    );
}
