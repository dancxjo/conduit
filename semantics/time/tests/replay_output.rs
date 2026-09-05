use conduit_core::{kind_id, TemporalInstant, TemporalScale};
use conduit_time::{
    decode_replay_event, decode_replay_state, encode_replay_event_into, encode_replay_state_into,
    OwnedReplayEvent, ReplayEmission, ReplayOutputCodecRefusal, ReplayState,
    MAXIMUM_REPLAY_ENTRIES, MAXIMUM_REPLAY_EVENT_BYTES, MAXIMUM_REPLAY_IDENTITY_BYTES,
    MAXIMUM_REPLAY_STATE_BYTES,
};
mod common;

fn historical_time(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "memory-lantern/event-clock".into(),
        resolution_ticks: 2,
        uncertainty_ticks: 1,
    }
}

#[test]
fn replay_event_round_trip_preserves_both_time_domains_and_identity() {
    let value = common::replay_value(7, "bench/record@1");
    let event = ReplayEmission {
        ordinal: 3,
        historical_sequence: 73,
        historical_identity: "bench/observation/amber",
        historical_event_time: &historical_time(1_250),
        historical_origin: conduit_time::HistoricalEntryOrigin::OperatorAuthored,
        value: &value,
        playback_ticks: 9_000,
    };
    let mut encoded = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let length = encode_replay_event_into(event, &mut encoded).unwrap();

    assert_eq!(
        decode_replay_event(&encoded[..length]),
        Ok(OwnedReplayEvent {
            ordinal: 3,
            historical_sequence: 73,
            historical_identity: "bench/observation/amber".into(),
            historical_event_time: historical_time(1_250),
            historical_origin: conduit_time::HistoricalEntryOrigin::OperatorAuthored,
            value,
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
    let time = historical_time(1);
    let value = common::replay_value(1, "bench/record@1");
    let event = |ordinal, identity| ReplayEmission {
        ordinal,
        historical_sequence: 1,
        historical_identity: identity,
        historical_event_time: &time,
        historical_origin: conduit_time::HistoricalEntryOrigin::MachineObservation,
        value: &value,
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
    let mut invalid_value = value.clone();
    invalid_value.content_profile = kind_id("");
    assert_eq!(
        encode_replay_event_into(
            ReplayEmission {
                value: &invalid_value,
                ..event(0, "event")
            },
            &mut encoded,
        ),
        Err(ReplayOutputCodecRefusal::InvalidResource)
    );
}

#[test]
fn malformed_values_remain_machine_readable_refusals() {
    let mut event = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let value = common::replay_value(1, "bench/record@1");
    let length = encode_replay_event_into(
        ReplayEmission {
            ordinal: 0,
            historical_sequence: 1,
            historical_identity: "event",
            historical_event_time: &historical_time(1),
            historical_origin: conduit_time::HistoricalEntryOrigin::MachineObservation,
            value: &value,
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
