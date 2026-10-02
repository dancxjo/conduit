use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    BeaconKind, BeaconObservation, ChargingObservation, ChargingState, CliffObservation,
    CliffSignal, BODY_SECTOR_FRONT_LEFT, BODY_SECTOR_LEFT, CHARGING_SOURCE_HOME_BASE,
};

#[test]
fn remaining_observations_round_trip_through_exact_native_types() {
    let beacon = BeaconObservation::new(BeaconKind::InfraredCode, 42).unwrap();
    let cliff = CliffObservation::new(
        BODY_SECTOR_FRONT_LEFT,
        BODY_SECTOR_LEFT | BODY_SECTOR_FRONT_LEFT,
        [0, 42, 0, 0],
    )
    .unwrap();
    let charging = ChargingObservation {
        state: ChargingState::Trickle,
        sources: CHARGING_SOURCE_HOME_BASE,
        millivolts: 14_200,
        milliamps: 240,
        temperature_celsius: 31,
        charge_mah: 1_200,
        capacity_mah: 2_400,
    }
    .new()
    .unwrap();

    let beacon_structured = beacon.into_structured().unwrap();
    assert_eq!(
        BeaconObservation::from_structured(beacon_structured).unwrap(),
        beacon
    );
    let cliff_structured = cliff.into_structured().unwrap();
    assert_eq!(
        CliffObservation::from_structured(cliff_structured).unwrap(),
        cliff
    );
    let charging_structured = charging.into_structured().unwrap();
    assert_eq!(
        ChargingObservation::from_structured(charging_structured).unwrap(),
        charging
    );
}

#[test]
fn native_contracts_preserve_observation_bounds_and_absence() {
    assert!(BeaconObservation::new_native(BeaconKind::VirtualWall, 1).is_err());
    assert!(ChargingObservation::new_native(ChargingState::Full, 0, 0, 0, 0, 2, 1,).is_err());

    let cliff = CliffObservation::new_native(
        BODY_SECTOR_LEFT,
        CliffSignal::observed(0).unwrap(),
        CliffSignal::unavailable(),
        CliffSignal::unavailable(),
        CliffSignal::unavailable(),
    )
    .unwrap();
    assert_eq!(cliff.signals(), (BODY_SECTOR_LEFT, [0, 0, 0, 0]));
    assert_eq!(CliffObservation::decode(&cliff.encode()), Ok(cliff));
}
