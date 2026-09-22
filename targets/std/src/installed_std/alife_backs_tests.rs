use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallOutcome,
};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn initialized_operation() -> LeniaStepBack {
    let seed = conduit_alife::orbium_seed(32, 32, 1)
        .unwrap()
        .encode()
        .unwrap();
    let mut operation = LeniaStepBack::new();
    let mut io = StepIo::test_frame(
        [Some(value(1, seed.len() as u32)), None],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        None,
        8,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&seed), None], None)
        ),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| (request.0, request.1)),
        Some((RequestId(0), HostCallId(0)))
    );
    let mut io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    operation
}

#[test]
fn value_closure_then_ordered_tick_is_the_only_accepted_lifecycle() {
    let mut operation = initialized_operation();
    let tick = super::super::contract::encode_tick(0);
    let tick_ref = value(2, tick.len() as u32);
    let mut io = StepIo::test_frame(
        [None, Some(tick_ref)],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        None,
        8,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&tick)], None)
        ),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| (request.0, request.1)),
        Some((RequestId(1), HostCallId(1)))
    );
    let field = BoundedValueRef::new(
        value(3, LENIA_MAXIMUM_FIELD_BYTES),
        LENIA_MAXIMUM_FIELD_BYTES,
    )
    .unwrap();
    let mut io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        Some((
            RequestId(1),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(field),
                failure: None,
            },
        )),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(field.value));
    let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
}

#[test]
fn reordered_tick_and_completion_after_cancel_remain_machine_readable_failures() {
    let mut reordered = initialized_operation();
    let tick = super::super::contract::encode_tick(1);
    let mut io = StepIo::test_frame(
        [None, Some(value(2, tick.len() as u32))],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        None,
        8,
    );
    assert_eq!(
        reordered.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&tick)], None)
        ),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 183,
        })
    );

    let mut cancelled = initialized_operation();
    let tick = super::super::contract::encode_tick(0);
    let mut io = StepIo::test_frame(
        [None, Some(value(2, tick.len() as u32))],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        None,
        8,
    );
    assert_eq!(
        cancelled.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&tick)], None)
        ),
        StepOutcome::Progress
    );
    StepBack::<2>::cancel(&mut cancelled);
    let mut io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [Some(LENIA_MAXIMUM_FIELD_BYTES), None],
        Some((
            RequestId(1),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )),
        8,
    );
    assert_eq!(
        cancelled.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 188,
        })
    );
}
