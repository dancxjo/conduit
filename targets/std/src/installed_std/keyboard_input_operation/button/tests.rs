use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallOutcome,
};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn source() -> ButtonOperation {
    ButtonOperation {
        empty: value(0, 0),
        next: 0,
        pending: None,
        terminal: false,
        validator: PreparedStructuredValueValidator::new(
            &conduit_semantic_catalog::input_button_transition_type(),
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES as usize,
        )
        .unwrap(),
    }
}

fn request(source: &mut ButtonOperation, expected: u32) {
    let mut io = StepIo::test_frame([None], [false], [Some(256)], None, 8);
    assert_eq!(
        source.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(expected))
    );
}

fn complete(
    source: &mut ButtonOperation,
    request: RequestId,
    outcome: HostCallOutcome,
    canonical: Option<&[u8]>,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        )],
        Some((request, outcome)),
        8,
    );
    let outcome = source.step(&mut io, &StepInputBytes::test_frame([None], canonical));
    (outcome, io)
}

fn transition(pressed: bool, sequence: u64) -> Vec<u8> {
    conduit_semantic_catalog::button_transition_value("button/primary", pressed, sequence)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn completed(output: ValueRef) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(
            BoundedValueRef::new(
                output,
                conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
            )
            .unwrap(),
        ),
        failure: None,
    }
}

#[test]
fn transitions_continue_in_one_play_until_explicit_stop() {
    let mut source = source();
    request(&mut source, 0);
    for (index, pressed) in [true, false, true, false, true, false]
        .into_iter()
        .enumerate()
    {
        let canonical = transition(pressed, index as u64);
        let output = value(2 + index as u16, canonical.len() as u32);
        let (outcome, io) = complete(
            &mut source,
            RequestId(u32::try_from(index).unwrap()),
            completed(output),
            Some(&canonical),
        );
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), Some(output));
        request(&mut source, u32::try_from(index).unwrap() + 1);
    }
    StepBack::<1>::cancel(&mut source);
    assert!(source.terminal);
    assert!(source.pending.is_none());
}

#[test]
fn malformed_failure_and_late_completion_remain_distinct() {
    let mut source = source();
    request(&mut source, 0);
    assert_eq!(
        complete(
            &mut source,
            RequestId(0),
            completed(value(2, 3)),
            Some(b"bad")
        )
        .0,
        button_step_fail(FailureCode::InvalidInput, 5)
    );

    let mut source = self::source();
    request(&mut source, 0);
    let failure = Failure {
        code: FailureCode::HostCallFailed,
        detail: 42,
    };
    assert_eq!(
        complete(
            &mut source,
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                failure: Some(failure),
                output: None,
            },
            None,
        )
        .0,
        StepOutcome::Fail(failure)
    );
    StepBack::<1>::cancel(&mut source);
    assert_eq!(
        complete(
            &mut source,
            RequestId(0),
            completed(value(3, 3)),
            Some(b"bad")
        )
        .0,
        StepOutcome::Complete
    );
}
