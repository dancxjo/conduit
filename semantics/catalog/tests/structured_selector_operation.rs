#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::StructuredSelectorOperation;

fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 4,
    }
}

fn input(operation: &mut StructuredSelectorOperation) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame([Some(value())], [false], [Some(4096)], None, 4);
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

fn completion(
    operation: &mut StructuredSelectorOperation,
    request: u32,
    output: bool,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(4096)],
        Some((
            RequestId(request),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: output.then(|| BoundedValueRef::new(value(), 4096).unwrap()),
                failure: None,
            },
        )),
        4,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

#[test]
fn dropped_value_does_not_close_flow_and_next_match_emits() {
    let mut operation = StructuredSelectorOperation::new(4096);
    for request in 0..2 {
        let (outcome, io) = input(&mut operation);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(
            io.test_host_request(),
            Some((
                RequestId(request),
                HostCallId(0),
                BoundedValueRef::new(value(), 4096).unwrap()
            ))
        );
        let (outcome, io) = completion(&mut operation, request, request == 1);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), (request == 1).then(value));
    }
    let mut io = StepIo::test_frame([None], [true], [None], None, 4);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn cancel_rejects_late_completion() {
    let mut operation = StructuredSelectorOperation::new(4096);
    input(&mut operation);
    StepOperation::<1>::cancel(&mut operation);
    assert_eq!(
        completion(&mut operation, 0, true).0,
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 143
        })
    );
}
