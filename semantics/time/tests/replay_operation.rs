use conduit_time::{
    decode_replay_event, decode_replay_state, encode_replay_command_into,
    encode_replay_timeline_into, BoundedReplayOperation, HistoricalReplayEntry, ReplayCommand,
    ReplayOperationOutput, ReplayOperationRefusal, ReplayPolicy, ReplayState,
    MAXIMUM_REPLAY_COMMAND_BYTES, MAXIMUM_REPLAY_EVENT_BYTES, MAXIMUM_REPLAY_STATE_BYTES,
    MAXIMUM_REPLAY_TIMELINE_BYTES,
};

fn timeline() -> Vec<u8> {
    let entries = [
        HistoricalReplayEntry {
            identity: "memory-lantern/amber".into(),
            event_ticks: 1_000,
        },
        HistoricalReplayEntry {
            identity: "memory-lantern/blue".into(),
            event_ticks: 1_250,
        },
    ];
    let mut encoded = vec![0; MAXIMUM_REPLAY_TIMELINE_BYTES];
    let length = encode_replay_timeline_into(&entries, &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

fn command(command: ReplayCommand) -> Vec<u8> {
    let mut encoded = vec![0; MAXIMUM_REPLAY_COMMAND_BYTES];
    let length = encode_replay_command_into(command, &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

fn outputs() -> (Vec<u8>, Vec<u8>) {
    (
        vec![0; MAXIMUM_REPLAY_EVENT_BYTES],
        vec![0; MAXIMUM_REPLAY_STATE_BYTES],
    )
}

#[test]
fn ordinary_values_drive_original_timing_without_merging_time_domains() {
    let mut operation = BoundedReplayOperation::new(ReplayPolicy::OriginalTiming).unwrap();
    operation.load_timeline(&timeline()).unwrap();
    let (mut event, mut state) = outputs();

    let started = operation
        .apply_command(
            &command(ReplayCommand::Start),
            9_000,
            &mut event,
            &mut state,
        )
        .unwrap();
    assert_eq!(started.event_bytes, None);
    assert_eq!(
        decode_replay_state(&state[..started.state_bytes.unwrap()]),
        Ok(ReplayState::Running)
    );

    let first = operation.poll(9_000, &mut event, &mut state).unwrap();
    let first_event = decode_replay_event(&event[..first.event_bytes.unwrap()]).unwrap();
    assert_eq!(first_event.historical_identity, "memory-lantern/amber");
    assert_eq!(first_event.historical_event_ticks, 1_000);
    assert_eq!(first_event.playback_ticks, 9_000);
    assert_eq!(
        decode_replay_state(&state[..first.state_bytes.unwrap()]),
        Ok(ReplayState::Running)
    );

    assert_eq!(
        operation.poll(9_249, &mut event, &mut state),
        Ok(ReplayOperationOutput {
            event_bytes: None,
            state_bytes: None
        })
    );
    let second = operation.poll(9_250, &mut event, &mut state).unwrap();
    let second_event = decode_replay_event(&event[..second.event_bytes.unwrap()]).unwrap();
    assert_eq!(second_event.historical_identity, "memory-lantern/blue");
    assert_eq!(second_event.historical_event_ticks, 1_250);
    assert_eq!(second_event.playback_ticks, 9_250);
    assert_eq!(
        decode_replay_state(&state[..second.state_bytes.unwrap()]),
        Ok(ReplayState::Completed)
    );
}

#[test]
fn admitted_output_failure_is_atomic_and_does_not_consume_an_event() {
    let mut operation = BoundedReplayOperation::new(ReplayPolicy::Step).unwrap();
    operation.load_timeline(&timeline()).unwrap();
    let (mut event, mut state) = outputs();
    operation
        .apply_command(&command(ReplayCommand::Start), 100, &mut event, &mut state)
        .unwrap();

    assert_eq!(
        operation.apply_command(
            &command(ReplayCommand::Step),
            101,
            &mut event[..MAXIMUM_REPLAY_EVENT_BYTES - 1],
            &mut state,
        ),
        Err(ReplayOperationRefusal::EventOutputTooSmall)
    );
    assert_eq!(operation.cursor(), Some(0));
    assert_eq!(operation.state(), Some(ReplayState::Paused));

    let stepped = operation
        .apply_command(&command(ReplayCommand::Step), 101, &mut event, &mut state)
        .unwrap();
    assert_eq!(
        decode_replay_event(&event[..stepped.event_bytes.unwrap()])
            .unwrap()
            .ordinal,
        0
    );
}

#[test]
fn malformed_control_and_active_timeline_replacement_fail_without_mutation() {
    let mut operation = BoundedReplayOperation::new(ReplayPolicy::OriginalTiming).unwrap();
    operation.load_timeline(&timeline()).unwrap();
    let (mut event, mut state) = outputs();
    operation
        .apply_command(&command(ReplayCommand::Start), 500, &mut event, &mut state)
        .unwrap();

    assert!(matches!(
        operation.apply_command(b"bad", 501, &mut event, &mut state),
        Err(ReplayOperationRefusal::Command(_))
    ));
    assert_eq!(operation.cursor(), Some(0));
    assert_eq!(operation.state(), Some(ReplayState::Running));
    assert_eq!(
        operation.load_timeline(&timeline()),
        Err(ReplayOperationRefusal::TimelineWhileActive)
    );
    assert_eq!(operation.cursor(), Some(0));
}

#[test]
fn missing_or_invalid_timeline_and_policy_remain_distinct() {
    assert!(matches!(
        BoundedReplayOperation::new(ReplayPolicy::Rate {
            numerator: 0,
            denominator: 1,
        }),
        Err(ReplayOperationRefusal::InvalidPolicy)
    ));
    let mut operation = BoundedReplayOperation::new(ReplayPolicy::Step).unwrap();
    let (mut event, mut state) = outputs();
    assert_eq!(
        operation.apply_command(&command(ReplayCommand::Start), 0, &mut event, &mut state),
        Err(ReplayOperationRefusal::MissingTimeline)
    );
    assert!(matches!(
        operation.load_timeline(b"bad"),
        Err(ReplayOperationRefusal::Timeline(_))
    ));
    assert_eq!(operation.state(), None);
}
