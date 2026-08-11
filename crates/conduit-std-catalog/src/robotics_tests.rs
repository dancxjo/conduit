use super::*;

#[test]
fn robotics_contracts_are_typed_bounded_simulated_and_authority_free() {
    for (contract, revision) in robotics_contracts_with_revisions() {
        let implementation = match contract.kind_id.as_str() {
            ROBOTICS_OBSERVE_BUMP_KIND => ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION,
            ROBOTICS_OBSERVE_IMU_KIND => ROBOTICS_OBSERVE_IMU_IMPLEMENTATION,
            ROBOTICS_OBSERVE_RANGE_KIND => ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION,
            ROBOTICS_OBSERVE_ODOMETRY_KIND => ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION,
            ROBOTICS_OBSERVE_BATTERY_KIND => ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION,
            ROBOTICS_VELOCITY_INTENT_KIND => ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION,
            ROBOTICS_DRIVE_DIFFERENTIAL_KIND => ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION,
            _ => unreachable!(),
        };
        let offer = offer(contract.clone(), revision, "test", implementation);
        assert!(contract
            .inputs
            .iter()
            .chain(&contract.outputs)
            .all(|port| port.value_kind.as_str() != crate::GENERIC_VALUE_KIND));
        assert_eq!(contract.limits.max_queue_items, 1);
        assert!(offer.host_operations.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
        assert!(offer
            .implementation
            .implementation_id
            .as_str()
            .contains("prewake"));
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
fn robotics_catalog_rejects_invalid_battery_and_range_configuration() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
    for source in [
        "form 0\n\ninvalid {\n battery: robotics/observe-battery\n battery.charge-permille = 1001\n}\n",
        "form 0\n\ninvalid {\n range: robotics/observe-range\n range.distance-mm = 1000001\n}\n",
    ] {
        assert!(conduit_form::parse(source, &profile).is_err());
    }
}
