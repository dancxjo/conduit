use super::*;

#[test]
fn robotics_contracts_are_typed_and_bounded() {
    for (contract, _) in robotics_contracts_with_revisions() {
        assert_eq!(contract.limits.max_queue_items, 1);
        assert!(contract.limits.max_queue_bytes > 0);
    }
}

#[test]
fn robotics_observations_use_distinct_exact_info_shapes() {
    let infos = [
        robotics_observe_bump_contract().outputs[0]
            .value_kind
            .clone(),
        robotics_observe_imu_contract().outputs[0]
            .value_kind
            .clone(),
        robotics_observe_range_contract().outputs[0]
            .value_kind
            .clone(),
        robotics_observe_odometry_contract().outputs[0]
            .value_kind
            .clone(),
        robotics_observe_battery_contract().outputs[0]
            .value_kind
            .clone(),
    ];
    assert_eq!(
        infos
            .iter()
            .collect::<alloc::collections::BTreeSet<_>>()
            .len(),
        infos.len()
    );
}

#[cfg(feature = "plot-catalog")]
#[test]
fn robotics_catalog_rejects_invalid_observation_and_motion_configuration() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
    for source in [
        "plot invalid {\n battery: robotics/observe-battery(charge-permille = 1001)\n}\n",
        "plot invalid {\n range: robotics/observe-range(distance = 1000001mm)\n}\n",
        "plot invalid {\n drive: robotics/drive-differential(ttl-ms = 9)\n}\n",
        "plot invalid {\n drive: robotics/drive-differential(minimum-clearance-mm = 250)\n}\n",
    ] {
        assert!(conduit_plot::parse(source, &profile).is_err());
    }
}

#[cfg(feature = "plot-catalog")]
#[test]
fn ordinary_robotics_plot_retains_typed_distance_quantity() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_plot::parse_syntax_document(include_str!(
        "../../../plots/robotics-range/main.conduit"
    ));
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .expect("ordinary robotics plot checks");
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "robotics-range", &profile)
            .expect("ordinary robotics plot expands");
    let distance = expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "distance")
        .expect("distance configuration exists");
    assert_eq!(
        distance.value,
        ConfigurationValue::Quantity(Quantity::new(500, QuantityUnit::Millimeter))
    );
}

#[test]
fn differential_drive_front_cannot_author_wire_around_local_safety() {
    let contract = robotics_drive_differential_contract();
    assert_eq!(
        contract
            .inputs
            .iter()
            .map(|port| port.port_id.as_str())
            .collect::<Vec<_>>(),
        ["linear", "angular"]
    );
    assert_eq!(contract.configuration.len(), 1);
    assert_eq!(contract.configuration[0].key, "ttl-ms");
    let authored_names = contract
        .inputs
        .iter()
        .map(|port| port.port_id.as_str())
        .chain(
            contract
                .configuration
                .iter()
                .map(|field| field.key.as_str()),
        )
        .collect::<Vec<_>>();
    for forbidden in ["bumper", "range", "cliff", "wheel-drop", "watchdog"] {
        assert!(!authored_names.contains(&forbidden));
    }
    assert!(contract
        .summary
        .contains("non-bypassable local safety and authority"));
    assert!(!contract.summary.to_ascii_lowercase().contains("simulated"));
}
