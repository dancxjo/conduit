use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{BeaconKind, ChargingState, RoboticsSimulationAvailability};

#[test]
fn robotics_variants_are_native_types_with_explicit_mechanism_tags() {
    for (value, tag) in [
        (ChargingState::NotCharging, 0),
        (ChargingState::Reconditioning, 1),
        (ChargingState::Full, 2),
        (ChargingState::Trickle, 3),
        (ChargingState::Waiting, 4),
        (ChargingState::Fault, 5),
    ] {
        assert_eq!(value.wire_tag(), tag);
        assert_eq!(ChargingState::try_from(tag).unwrap(), value);
        let encoded = NativeRustBinding::encode(value).unwrap();
        assert_eq!(ChargingState::decode(&encoded).unwrap(), value);
    }

    for (value, tag) in [(BeaconKind::VirtualWall, 0), (BeaconKind::InfraredCode, 1)] {
        assert_eq!(value.wire_tag(), tag);
        let encoded = NativeRustBinding::encode(value).unwrap();
        assert_eq!(BeaconKind::decode(&encoded).unwrap(), value);
    }
}

#[test]
fn simulation_availability_round_trips_through_its_exact_native_type() {
    for availability in [
        RoboticsSimulationAvailability::Fresh,
        RoboticsSimulationAvailability::Missing,
        RoboticsSimulationAvailability::Stale,
    ] {
        let structured = availability.into_structured().unwrap();
        assert_eq!(
            RoboticsSimulationAvailability::from_structured(structured).unwrap(),
            availability
        );
    }
}
