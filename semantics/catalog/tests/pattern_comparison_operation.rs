#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    PortId, RequestId, ValueRef,
};
use conduit_semantic_catalog::PatternComparisonOperation;

fn value(byte_len: u32) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len,
    }
}

fn input(
    operation: &mut PatternComparisonOperation,
    port: usize,
    value: ValueRef,
) -> (StepOutcome, StepIo<2>) {
    let mut inputs = [None; 2];
    inputs[port] = Some(value);
    let mut io = StepIo::test_frame(inputs, [false; 2], [Some(4096), None], None, 4);
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None));
    (outcome, io)
}

fn completion(
    operation: &mut PatternComparisonOperation,
    request: RequestId,
    output: Option<BoundedValueRef>,
) -> (StepOutcome, StepIo<2>) {
    let mut io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [Some(4096), None],
        Some((
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output,
                failure: None,
            },
        )),
        4,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None));
    (outcome, io)
}

#[test]
fn host_input_bound_is_exact_and_oversize_refuses() {
    for bound in [4096, 65536] {
        let mut operation = PatternComparisonOperation::new(bound);
        let (outcome, io) = input(&mut operation, 0, value(bound));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(
            io.test_host_request(),
            Some((
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(value(bound), bound).unwrap()
            ))
        );
        let mut operation = PatternComparisonOperation::new(bound);
        assert_eq!(
            input(&mut operation, 0, value(bound + 1)).0,
            StepOutcome::Fail(Failure {
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
        for (index, port) in ports.into_iter().enumerate() {
            let request = RequestId(index as u32);
            let (outcome, io) = input(&mut operation, port, value(10));
            assert_eq!(outcome, StepOutcome::Progress);
            assert_eq!(
                io.test_host_request().map(|request| (request.0, request.1)),
                Some((request, HostCallId(port as u16)))
            );
            let output = (index == 1).then(|| BoundedValueRef::new(value(20), 4096).unwrap());
            let (outcome, io) = completion(&mut operation, request, output);
            assert_eq!(outcome, StepOutcome::Progress);
            assert_eq!(io.test_output(PortId(0)), (index == 1).then(|| value(20)));
        }
        let (outcome, io) = input(&mut operation, 0, value(10));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(
            io.test_host_request().map(|request| (request.0, request.1)),
            Some((RequestId(2), HostCallId(0)))
        );
        let (outcome, io) = completion(
            &mut operation,
            RequestId(2),
            Some(BoundedValueRef::new(value(20), 4096).unwrap()),
        );
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), Some(value(20)));

        let mut close_first = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 4);
        assert_eq!(
            operation.step(
                &mut close_first,
                &StepInputBytes::test_frame([None; 2], None)
            ),
            StepOutcome::Progress
        );
        let mut close_second = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 4);
        assert_eq!(
            operation.step(
                &mut close_second,
                &StepInputBytes::test_frame([None; 2], None)
            ),
            StepOutcome::Complete
        );
    }
}

#[test]
fn stale_completion_and_cancelled_work_do_not_emit() {
    let mut operation = PatternComparisonOperation::new(4096);
    input(&mut operation, 0, value(10));
    StepOperation::<2>::cancel(&mut operation);
    assert_eq!(
        completion(
            &mut operation,
            RequestId(0),
            Some(BoundedValueRef::new(value(20), 4096).unwrap()),
        )
        .0,
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 251
        })
    );
}
