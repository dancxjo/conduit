use super::*;
use conduit_kernel::{HostCallOutcome, ProtocolError};

fn value(slot: u16, bytes: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes,
    }
}

fn fresh_operation() -> RhythmCompareOperation {
    RhythmCompareOperation {
        pending: None,
        next_request: 0,
        drain_marker: value(99, 0),
        release_drain_marker: false,
        closed: [false; 2],
        draining_missed: false,
    }
}

fn completed(output: Option<ValueRef>) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: output.map(|value| BoundedValueRef::new(value, 4096).unwrap()),
        failure: None,
    }
}

fn frame(
    inputs: [Option<ValueRef>; 2],
    closed: [bool; 2],
    completion: Option<(RequestId, HostCallOutcome)>,
) -> StepIo<2> {
    StepIo::test_frame(inputs, closed, [Some(4096), None], completion, 8)
}

fn run(operation: &mut RhythmCompareOperation, io: &mut StepIo<2>) -> StepOutcome {
    operation.step(io, &StepInputBytes::test_frame([None; 2], None))
}

#[test]
fn exact_ports_request_admitted_host_calls_without_retaining_inputs() {
    let mut operation = fresh_operation();
    let performance = value(1, 43);
    let mut io = frame([Some(performance), None], [false; 2], None);
    assert_eq!(run(&mut operation, &mut io), StepOutcome::Progress);
    assert!(io.test_consumed(PortId(0)));
    assert_eq!(
        io.test_host_request().map(|request| (request.0, request.1)),
        Some((RequestId(0), HostCallId(1)))
    );

    let mut blocked = frame([None, Some(value(2, 100))], [false; 2], None);
    assert!(matches!(
        run(&mut operation, &mut blocked),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert!(!blocked.test_consumed(PortId(1)));

    let mut completion = frame([None; 2], [false; 2], Some((RequestId(0), completed(None))));
    assert_eq!(run(&mut operation, &mut completion), StepOutcome::Progress);
    let reference = value(2, 512);
    let mut io = frame([None, Some(reference)], [false; 2], None);
    assert_eq!(run(&mut operation, &mut io), StepOutcome::Progress);
    assert!(io.test_consumed(PortId(1)));
    assert_eq!(
        io.test_host_request().map(|request| (request.0, request.1)),
        Some((RequestId(1), HostCallId(2)))
    );
}

#[test]
fn performance_close_drains_missed_feedback_before_exact_completion() {
    let mut operation = fresh_operation();
    let mut close_performance = frame([None; 2], [true, false], None);
    assert_eq!(
        run(&mut operation, &mut close_performance),
        StepOutcome::Progress
    );
    assert!(close_performance.test_consumed_closed(PortId(0)));
    assert_eq!(
        close_performance
            .test_host_request()
            .map(|request| (request.0, request.1)),
        Some((RequestId(0), HostCallId(0)))
    );

    let feedback = value(2, 800);
    let mut feedback_completion = frame(
        [None; 2],
        [false; 2],
        Some((RequestId(0), completed(Some(feedback)))),
    );
    assert_eq!(
        run(&mut operation, &mut feedback_completion),
        StepOutcome::Progress
    );
    assert_eq!(feedback_completion.test_output(PortId(0)), Some(feedback));
    assert_eq!(
        feedback_completion
            .test_host_request()
            .map(|request| (request.0, request.1)),
        Some((RequestId(1), HostCallId(0)))
    );

    let mut drained = frame([None; 2], [false; 2], Some((RequestId(1), completed(None))));
    assert_eq!(run(&mut operation, &mut drained), StepOutcome::Progress);
    let mut close_reference = frame([None; 2], [false, true], None);
    assert_eq!(
        run(&mut operation, &mut close_reference),
        StepOutcome::Complete
    );
    assert!(close_reference.test_consumed_closed(PortId(1)));
    assert!(close_reference
        .test_discards()
        .contains(&Some(value(99, 0))));
}

#[test]
fn oversized_value_and_wrong_completion_fail_closed() {
    let oversized = value(1, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 + 1);
    let mut operation = fresh_operation();
    let mut io = frame([Some(oversized), None], [false; 2], None);
    assert!(matches!(
        run(&mut operation, &mut io),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            ..
        })
    ));
    assert_eq!(
        BoundedValueRef::new(oversized, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
        Err(ProtocolError::HostCallInputExceeded)
    );

    let mut operation = fresh_operation();
    let mut wrong = frame([None; 2], [false; 2], Some((RequestId(7), completed(None))));
    assert!(matches!(
        run(&mut operation, &mut wrong),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
}
