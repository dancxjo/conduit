use conduit_time::{
    decode_replay_event, decode_replay_state, encode_replay_event_into, encode_replay_state_into,
    OwnedReplayEvent, ReplayEmission, ReplayOutputCodecRefusal, ReplayState,
    MAXIMUM_REPLAY_ENTRIES, MAXIMUM_REPLAY_EVENT_BYTES, MAXIMUM_REPLAY_IDENTITY_BYTES,
    MAXIMUM_REPLAY_STATE_BYTES,
};

#[test]
fn replay_event_round_trip_preserves_both_time_domains_and_identity() {
    let event = ReplayEmission {
        ordinal: 3,
        historical_identity: "bench/observation/amber",
        historical_event_ticks: 1_250,
        playback_ticks: 9_000,
    };
    let mut encoded = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let length = encode_replay_event_into(event, &mut encoded).unwrap();

    assert_eq!(
        decode_replay_event(&encoded[..length]),
        Ok(OwnedReplayEvent {
            ordinal: 3,
            historical_identity: "bench/observation/amber".into(),
            historical_event_ticks: 1_250,
            playback_ticks: 9_000,
        })
    );
}

#[test]
fn every_replay_state_has_an_exact_round_trip() {
    for state in [
        ReplayState::Stopped,
        ReplayState::Running,
        ReplayState::Paused,
        ReplayState::Completed,
        ReplayState::Failed { code: 77 },
    ] {
        let mut encoded = [0; MAXIMUM_REPLAY_STATE_BYTES];
        let length = encode_replay_state_into(state, &mut encoded).unwrap();
        assert_eq!(decode_replay_state(&encoded[..length]), Ok(state));
    }
}

#[test]
fn event_encoding_refuses_invalid_identity_ordinal_and_capacity() {
    let mut encoded = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let event = |ordinal, identity| ReplayEmission {
        ordinal,
        historical_identity: identity,
        historical_event_ticks: 1,
        playback_ticks: 2,
    };

    assert_eq!(
        encode_replay_event_into(event(MAXIMUM_REPLAY_ENTRIES, "event"), &mut encoded),
        Err(ReplayOutputCodecRefusal::OrdinalOutOfBounds)
    );
    assert_eq!(
        encode_replay_event_into(event(0, ""), &mut encoded),
        Err(ReplayOutputCodecRefusal::EmptyIdentity)
    );
    let long_identity = "x".repeat(MAXIMUM_REPLAY_IDENTITY_BYTES + 1);
    assert_eq!(
        encode_replay_event_into(event(0, &long_identity), &mut encoded),
        Err(ReplayOutputCodecRefusal::IdentityTooLong)
    );
    assert_eq!(
        encode_replay_event_into(event(0, "event"), &mut [0; 29]),
        Err(ReplayOutputCodecRefusal::OutputTooSmall)
    );
}

#[test]
fn malformed_values_remain_machine_readable_refusals() {
    let mut event = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let length = encode_replay_event_into(
        ReplayEmission {
            ordinal: 0,
            historical_identity: "event",
            historical_event_ticks: 1,
            playback_ticks: 2,
        },
        &mut event,
    )
    .unwrap();
    assert_eq!(
        decode_replay_event(&event[..length - 1]),
        Err(ReplayOutputCodecRefusal::Truncated)
    );
    event[4] = 2;
    assert_eq!(
        decode_replay_event(&event[..length]),
        Err(ReplayOutputCodecRefusal::UnsupportedVersion)
    );

    let mut state = [0; MAXIMUM_REPLAY_STATE_BYTES];
    let length = encode_replay_state_into(ReplayState::Running, &mut state).unwrap();
    state[5] = 99;
    assert_eq!(
        decode_replay_state(&state[..length]),
        Err(ReplayOutputCodecRefusal::UnknownState)
    );
}
