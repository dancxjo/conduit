use conduit_thermostat_app::execution::Execution;
use conduit_thermostat_plot::{Command, Fan, Mode, Preset, ThermostatState};

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
        let result = execution.execute(state, command).unwrap();
        assert_ne!(previous_play.as_ref(), Some(&result.basis.active_play_id));
        if let Some(plan) = &expected_plan {
            assert_eq!(plan, &result.basis.plan_id);
        }
        expected_plan = Some(result.basis.plan_id.clone());
        previous_play = Some(result.basis.active_play_id.clone());
        assert_eq!(result.state, state.apply(command).unwrap());
        state = result.state;
    }
    let basis = execution
        .execute(state, Command::SetMode(Mode::Off))
        .unwrap()
        .basis;
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({"plan_id":basis.plan_id, "play_id":basis.active_play_id})
    );
    assert!(execution.execute(state, Command::SetTarget(301)).is_err());
    assert_eq!(state.target, 190);
}
