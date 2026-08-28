#[test]
fn human_semantics_depend_only_downward() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "apps/",
        "targets/",
        "proof/",
        "conduit-std-host",
        "patchbay",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "portable human semantics must not depend upward on {forbidden}"
        );
    }
}

#[test]
fn exact_human_value_identities_remain_stable() {
    assert_eq!(conduit_human::TEXT_INFO_ID, "value/text@1");
    assert_eq!(conduit_human::KEY_EVENT_INFO_ID, "input/key-event@1");
    assert_eq!(conduit_human::CHORD_INFO_ID, "input/chord@1");
}
