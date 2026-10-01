#[test]
fn robotics_semantics_depend_only_downward() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "products/",
        "bodies/",
        "targets/",
        "proof/",
        "conduit-std-host",
        "conduit-pete",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "portable robotics semantics must not depend upward on {forbidden}"
        );
    }
}

#[test]
fn exact_robotics_value_identities_remain_stable() {
    assert_eq!(
        conduit_robotics::ROBOTICS_RANGE_INFO_ID,
        "robotics/range-mm-sensor-forward@1"
    );
    assert_eq!(
        conduit_robotics::ROBOTICS_CONTACT_INFO_ID,
        "robotics/contact-body-sectors@1"
    );
    assert_eq!(
        conduit_robotics::ROBOTICS_PROXIMITY_INFO_ID,
        "robotics/proximity-body-sectors@1"
    );
}

#[test]
fn remaining_portable_observations_are_generated_not_handwritten() {
    let input = include_str!("../src/input_info.rs");
    let hazards = include_str!("../src/hazard_info.rs");
    for declaration in [
        "pub struct BeaconObservation",
        "pub struct CliffObservation",
        "pub struct ChargingObservation",
    ] {
        assert!(!input.contains(declaration));
        assert!(!hazards.contains(declaration));
    }

    let types = include_str!("../types.conduit");
    for declaration in [
        "type BeaconObservation =",
        "type CliffObservation =",
        "type ChargingObservation =",
    ] {
        assert!(types.contains(declaration));
    }
}
