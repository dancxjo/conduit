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

#[cfg(feature = "form-catalog")]
#[test]
fn robotics_catalog_rejects_invalid_observation_and_motion_configuration() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
    for source in [
        "form invalid {\n battery: robotics/observe-battery(charge-permille = 1001)\n}\n",
        "form invalid {\n range: robotics/observe-range(distance-mm = 1000001)\n}\n",
        "form invalid {\n drive: robotics/drive-differential(ttl-ms = 9)\n}\n",
        "form invalid {\n drive: robotics/drive-differential(minimum-clearance-mm = 250)\n}\n",
    ] {
        assert!(conduit_form::parse(source, &profile).is_err());
    }
}

#[test]
fn differential_drive_face_cannot_author_wire_around_local_safety() {
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
