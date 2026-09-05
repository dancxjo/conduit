use conduit_time::*;
mod common;

fn entry(identity: &str, ticks: u64) -> HistoricalReplayEntry {
    HistoricalReplayEntry {
        sequence: ticks,
        identity: identity.into(),
        event_time: conduit_core::TemporalInstant {
            ticks,
            scale: conduit_core::TemporalScale::Milliseconds,
            clock_basis: "history-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        origin: HistoricalEntryOrigin::MachineObservation,
        value: common::replay_value(1, "bench/record@1"),
    }
}

fn entries() -> Vec<HistoricalReplayEntry> {
    vec![entry("event/a", 10), entry("event/b", 20)]
}

#[test]
fn every_replay_command_round_trips_through_the_finite_value() {
    for command in [
        ReplayCommand::Start,
        ReplayCommand::Stop,
        ReplayCommand::Pause,
        ReplayCommand::Resume,
        ReplayCommand::Restart,
        ReplayCommand::Step,
        ReplayCommand::Fail { code: 513 },
    ] {
        let mut encoded = [0; MAXIMUM_REPLAY_COMMAND_BYTES];
        let length = encode_replay_command_into(command, &mut encoded).unwrap();
        assert!(length <= MAXIMUM_REPLAY_COMMAND_BYTES);
        assert_eq!(decode_replay_command(&encoded[..length]), Ok(command));
    }
}

#[test]
fn command_codec_failures_remain_distinct() {
    let mut encoded = [0; MAXIMUM_REPLAY_COMMAND_BYTES];
    assert_eq!(
        encode_replay_command_into(ReplayCommand::Start, &mut encoded[..5]),
        Err(ReplayCommandCodecRefusal::OutputTooSmall)
    );
    let length = encode_replay_command_into(ReplayCommand::Fail { code: 7 }, &mut encoded).unwrap();
    assert_eq!(
        decode_replay_command(&encoded[..5]),
        Err(ReplayCommandCodecRefusal::Truncated)
    );
    let mut invalid = encoded;
    invalid[0] = 0;
    assert_eq!(
        decode_replay_command(&invalid[..length]),
        Err(ReplayCommandCodecRefusal::InvalidMagic)
    );
    let mut invalid = encoded;
    invalid[4] = 2;
    assert_eq!(
        decode_replay_command(&invalid[..length]),
        Err(ReplayCommandCodecRefusal::UnsupportedVersion)
    );
    let mut invalid = encoded;
    invalid[5] = 99;
    assert_eq!(
        decode_replay_command(&invalid[..length]),
        Err(ReplayCommandCodecRefusal::UnknownCommand)
    );
    assert_eq!(
        decode_replay_command(&encoded[..7]),
        Err(ReplayCommandCodecRefusal::Truncated)
    );
    let length = encode_replay_command_into(ReplayCommand::Start, &mut encoded).unwrap();
    encoded[length] = 1;
    assert_eq!(
        decode_replay_command(&encoded[..length + 1]),
        Err(ReplayCommandCodecRefusal::TrailingBytes)
    );
}

#[test]
fn explicit_commands_drive_step_stop_restart_and_failure() {
    let mut replay = BoundedReplayController::new(&entries(), ReplayPolicy::Step).unwrap();
    assert_eq!(replay.apply(ReplayCommand::Start, 100).unwrap(), None);
    let first = replay.apply(ReplayCommand::Step, 101).unwrap().unwrap();
    assert_eq!(first.historical_identity, "event/a");
    assert_eq!(replay.apply(ReplayCommand::Stop, 102).unwrap(), None);
    assert_eq!(replay.state(), ReplayState::Stopped);
    assert_eq!(replay.cursor(), 1);
    assert_eq!(replay.apply(ReplayCommand::Start, 200).unwrap(), None);
    assert_eq!(
        replay
            .apply(ReplayCommand::Step, 201)
            .unwrap()
            .unwrap()
            .historical_identity,
        "event/b"
    );
    assert_eq!(replay.state(), ReplayState::Completed);

    assert_eq!(replay.apply(ReplayCommand::Restart, 300).unwrap(), None);
    assert_eq!(replay.state(), ReplayState::Stopped);
    assert_eq!(replay.cursor(), 0);
    assert_eq!(replay.apply(ReplayCommand::Start, 300).unwrap(), None);
    assert_eq!(replay.apply(ReplayCommand::Fail { code: 9 }, 301), Ok(None));
    assert_eq!(replay.state(), ReplayState::Failed { code: 9 });
}
