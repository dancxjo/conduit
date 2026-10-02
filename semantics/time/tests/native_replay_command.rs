use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{ReplayCommand, ReplayState};

#[test]
fn replay_commands_round_trip_through_the_exact_native_type() {
    for command in [
        ReplayCommand::Start,
        ReplayCommand::Stop,
        ReplayCommand::Pause,
        ReplayCommand::Resume,
        ReplayCommand::Restart,
        ReplayCommand::Step,
        ReplayCommand::fail(0).unwrap(),
        ReplayCommand::fail(u16::MAX).unwrap(),
    ] {
        let structured = command.clone().into_structured().unwrap();
        assert_eq!(ReplayCommand::from_structured(structured).unwrap(), command);
    }
}

#[test]
fn replay_states_round_trip_through_the_exact_native_type() {
    for state in [
        ReplayState::Stopped,
        ReplayState::Running,
        ReplayState::Paused,
        ReplayState::Completed,
        ReplayState::failed(0).unwrap(),
        ReplayState::failed(u16::MAX).unwrap(),
    ] {
        let structured = state.into_structured().unwrap();
        assert_eq!(ReplayState::from_structured(structured).unwrap(), state);
    }
}
