use conduit_thermostat_plot::{Command, Fan, Mode, Preset, ThermostatState};
use conduit_thermostat_runtime::execution::Execution;

#[test]
fn authored_thermostat_commands_execute_exact_plan_and_play() {
    let mut execution = Execution::new().unwrap();
    let mut state = ThermostatState::default();
    let mut previous_play = None;
    let mut expected_plan = None;
    for command in [
        Command::SetTarget(225),
        Command::SetMode(Mode::Auto),
        Command::SetFan(Fan::On),
        Command::SetPreset(Preset::Sleep),
        Command::Observe(Some(170)),
        Command::Observe(None),
    ] {
        let result = execution.execute(command).unwrap();
        if let Some(play) = &previous_play {
            assert_eq!(play, &result.basis.active_play_id);
        }
        if let Some(plan) = &expected_plan {
            assert_eq!(plan, &result.basis.plan_id);
        }
        expected_plan = Some(result.basis.plan_id.clone());
        previous_play = Some(result.basis.active_play_id.clone());
        assert_eq!(result.state, state.apply(command).unwrap());
        state = result.state;
    }
    let basis = execution
        .execute(Command::SetMode(Mode::Off))
        .unwrap()
        .basis;
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({"plan_id":basis.plan_id, "play_id":basis.active_play_id})
    );
    let report = execution.finish().unwrap();
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(report.play.active_play_id, basis.active_play_id);
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 7);
    assert!(receipts
        .iter()
        .all(|receipt| receipt.parent_active_play_id == report.play.active_play_id));
    assert_eq!(state.target, 190);
}

#[test]
fn stopping_thermostat_terminates_the_native_body_play() {
    let mut execution = Execution::new().unwrap();
    let output = execution.execute(Command::SetMode(Mode::Heat)).unwrap();
    let report = execution.stop().unwrap();
    assert_eq!(report.play.active_play_id, output.basis.active_play_id);
    assert!(matches!(
        report.terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    assert!(report.live_fore_status.unwrap().play_terminal);
    assert!(!report.scan_cancellation_failed);
}
