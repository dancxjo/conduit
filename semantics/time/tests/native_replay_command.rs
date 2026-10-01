use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::ReplayCommand;

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
