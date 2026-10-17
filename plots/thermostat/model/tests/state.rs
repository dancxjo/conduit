use conduit_thermostat_plot::*;
#[test]
fn commands_preserve_units_bounds_and_preset_meaning() {
    let initial = ThermostatState::default();
    assert_eq!(
        ThermostatState::decode(&initial.encode().unwrap()),
        Ok(initial)
    );
    let commands = [
        Command::SetTarget(215),
        Command::SetMode(Mode::Heat),
        Command::SetFan(Fan::On),
        Command::SetPreset(Preset::Eco),
        Command::Observe(Some(150)),
        Command::Observe(None),
    ];
    let mut state = initial;
    for command in commands {
        assert_eq!(Command::decode(&command.encode()), Ok(command));
        state = state.apply(command).unwrap();
        assert_eq!(ThermostatState::decode(&state.encode().unwrap()), Ok(state));
    }
    assert_eq!(state.target, 180);
    assert_eq!(state.preset, Preset::Eco);
    assert_eq!(state.revision, 6);
    assert_eq!(
        initial.apply(Command::SetTarget(305)),
        Err(Refusal::TargetRange)
    );
    assert_eq!(
        initial.apply(Command::SetTarget(211)),
        Err(Refusal::TargetRange)
    );
    assert_eq!(
        initial.apply(Command::Observe(Some(801))),
        Err(Refusal::ObservationRange)
    );
    assert_eq!(
        initial.apply(Command::SetPreset(Preset::Custom)),
        Err(Refusal::InvalidCommand)
    );
    assert_eq!(initial.apply(Command::SetMode(Mode::Off)), Ok(initial));
}
#[test]
fn missing_sensor_and_demand_never_assert_equipment_effect() {
    let state = ThermostatState::default()
        .apply(Command::SetMode(Mode::Auto))
        .unwrap();
    assert_eq!(state.demand(), "Waiting for a temperature sensor");
    assert_eq!(
        state.apply(Command::Observe(Some(200))).unwrap().demand(),
        "Heating requested"
    );
    assert_eq!(
        state.apply(Command::Observe(Some(220))).unwrap().demand(),
        "Cooling requested"
    );
    assert_eq!(
        state.apply(Command::Observe(Some(210))).unwrap().demand(),
        "Within target deadband"
    );
    assert_eq!(
        state.apply(Command::Observe(Some(205))).unwrap().demand(),
        "Within target deadband"
    );
    assert_eq!(
        state.apply(Command::SetMode(Mode::Off)).unwrap().demand(),
        "Control is off"
    );
}
#[test]
fn exact_forms_refuse_trailing_bytes_unknown_tags_and_noncanonical_none() {
    for tag in 6..=255 {
        assert!(Command::decode(&[tag, 0, 0]).is_err());
    }
    assert!(Command::decode(&[5, 1, 0]).is_err());
    assert!(Command::decode(&[1, 1, 1]).is_err());
    assert!(Command::decode(&[0, 210, 0, 0]).is_err());
    let mut bytes = ThermostatState::default().encode().unwrap();
    bytes[11] = 1;
    assert!(ThermostatState::decode(&bytes).is_err());
    let state = ThermostatState {
        revision: u32::MAX,
        ..ThermostatState::default()
    };
    assert_eq!(
        state.apply(Command::SetTarget(220)),
        Err(Refusal::RevisionExhausted)
    );
    assert_eq!(state.apply(Command::SetMode(Mode::Off)), Ok(state));
}

#[test]
fn cooling_presets_relax_cooling_and_custom_targets_survive_mode_changes() {
    let cooling = ThermostatState::default()
        .apply(Command::SetMode(Mode::Cool))
        .unwrap();
    assert_eq!(cooling.target, 240);
    assert_eq!(
        cooling
            .apply(Command::SetPreset(Preset::Eco))
            .unwrap()
            .target,
        260
    );
    assert_eq!(
        cooling
            .apply(Command::SetPreset(Preset::Sleep))
            .unwrap()
            .target,
        250
    );
    let custom = cooling
        .apply(Command::SetTarget(225))
        .unwrap()
        .apply(Command::SetMode(Mode::Heat))
        .unwrap();
    assert_eq!(custom.target, 225);
    assert_eq!(custom.preset, Preset::Custom);
    assert_eq!(
        cooling.apply(Command::SetMode(Mode::Heat)).unwrap().target,
        210
    );
}
