#[test]
fn robotics_semantics_depend_only_downward() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "apps/",
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
