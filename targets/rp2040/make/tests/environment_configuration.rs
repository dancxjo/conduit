use conduit_host_make::*;
const SOURCE: &str = include_str!("../../profiles/pico-environment-usb.host.conduit");
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let config = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(config, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
#[test]
fn canonical_identity_and_executable_pin_selection_consume_the_same_checked_source() {
    let a = checked(SOURCE).unwrap();
    let rust = conduit_host_rp2040::environment_usb_rust(&a).unwrap();
    assert!(rust.contains(a.configuration_id()));
    for expected in [
        "I2C1",
        "PIN_6",
        "PIN_7",
        "SENSOR_ADDRESS:u8=118",
        "DISPLAY_ADDRESS:u8=112",
        "BRIGHTNESS:u8=3",
    ] {
        assert!(rust.contains(expected), "{expected}")
    }
    let round = canonical_host_configuration_conduit(a.configuration()).unwrap();
    let b = checked(&round).unwrap();
    assert_eq!(a.configuration_id(), b.configuration_id());
    let mut reversed = a.configuration().clone();
    reversed.devices.reverse();
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let c = check_host_configuration(
        reversed,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap();
    assert_eq!(a.configuration_id(), c.configuration_id());
    let other = SOURCE.replace(
        "controller: 1, sda: 6, scl: 7",
        "controller: 0, sda: 0, scl: 1",
    );
    let b = checked(&other).unwrap();
    assert_ne!(a.configuration_id(), b.configuration_id());
    let rust = conduit_host_rp2040::environment_usb_rust(&b).unwrap();
    assert!(rust.contains("$p.I2C0,$p.PIN_1,$p.PIN_0"));
}
#[test]
fn generic_shared_schema_refuses_unknown_fields_duplicates_and_conflicts() {
    for bad in [
        SOURCE.replace("frequency_hz: 100000", "frequency_hz: 100000, mystery: 3"),
        SOURCE.replace("address: 118", "address: 118, address: 119"),
        SOURCE.replace("kind: \"i2c\", id: \"sensor\"", "kind: \"unknown\", id: \"sensor\""),
        SOURCE.replace("brightness: 3", "brightness: 3, brightness: 4"),
        SOURCE.replace("brightness: 3", "brightness: {nested: 3}"),
        SOURCE.replace("brightness: 3", "brightness: [3]"),
        SOURCE.replace("address: 112", "address: 118"),
        SOURCE.replace("address: 112", "address: 128"),
        SOURCE.replace("bus: \"environment\", address: 118", "bus: \"missing\", address: 118"),
        SOURCE.replace("  limits =", "  bus = {kind: \"i2c\", id: \"other\", controller: 0, sda: 6, scl: 9, frequency_hz: 100000}\n  limits ="),
    ] { assert!(checked(&bad).is_err(), "accepted {bad}"); }
}
#[test]
fn driver_parameters_are_checked_by_rp2040_make_without_shared_chip_vocabulary() {
    for bad in [
        SOURCE.replace("bosch-environment@1", "unknown-device@1"),
        SOURCE.replace("layout: \"alpha4\"", "layout: \"unknown\""),
        SOURCE.replace("brightness: 3", "brightness: 16"),
        SOURCE.replace("brightness: 3", "brightness: true"),
        SOURCE.replace("brightness: 3", "brightness: \"3\""),
        SOURCE.replace("brightness: 3, ", ""),
        SOURCE.replace("brightness: 3", "brightness: 3, unused: true"),
        SOURCE.replace(
            "implementation: \"bosch-environment@1\"",
            "implementation: \"bosch-environment@1\", configuration: {unused: 1}",
        ),
    ] {
        let c = checked(&bad).unwrap();
        assert!(
            conduit_host_rp2040::environment_usb_rust(&c).is_err(),
            "accepted {bad}"
        );
    }
    let generic = SOURCE.replace("bosch-environment@1", "future-generic-driver@3")
        .replace("implementation: \"future-generic-driver@3\"", "implementation: \"future-generic-driver@3\", configuration: {enabled: true, count: 18446744073709551615, label: \"room sensor\"}");
    let c = checked(&generic).unwrap();
    let encoded = canonical_host_configuration_conduit(c.configuration()).unwrap();
    assert_eq!(
        c.configuration_id(),
        checked(&encoded).unwrap().configuration_id()
    );
    assert_ne!(
        c.configuration_id(),
        checked(&generic.replace("enabled: true", "enabled: false"))
            .unwrap()
            .configuration_id()
    );
    assert!(conduit_host_rp2040::environment_usb_rust(&c).is_err());
}
#[test]
fn rp2040_rejects_wrong_controller_pairs_reserved_pins_and_device_addresses() {
    for bad in [
        SOURCE.replace("controller: 1", "controller: 0"),
        SOURCE.replace("sda: 6, scl: 7", "sda: 7, scl: 6"),
        SOURCE.replace("sda: 6, scl: 7", "sda: 22, scl: 23"),
        SOURCE.replace("controller: 1", "controller: 2"),
        SOURCE.replace("address: 118", "address: 117"),
    ] {
        let c = checked(&bad).unwrap();
        assert!(conduit_host_rp2040::environment_usb_rust(&c).is_err());
    }
}
#[test]
fn old_empty_attachment_configurations_keep_their_legacy_identity_encoding() {
    let source = include_str!("../../profiles/pico-w.host.conduit");
    let c = checked(source).unwrap();
    assert!(c.configuration().buses.is_empty() && c.configuration().devices.is_empty());
    let canonical = canonical_host_configuration_conduit(c.configuration()).unwrap();
    assert!(!canonical.contains("  bus =") && !canonical.contains("  device ="));
    assert_eq!(
        c.configuration_id(),
        checked(&canonical).unwrap().configuration_id()
    );
}

#[test]
fn pico_w_wireless_permission_is_checked_target_bound_not_a_usb_alias() {
    let plain = checked(SOURCE).unwrap();
    let source = include_str!("../../profiles/pico-w-environment-usb.host.conduit");
    let wireless = checked(source).unwrap();
    assert_ne!(plain.configuration_id(), wireless.configuration_id());
    let plain = conduit_host_rp2040::environment_usb_rust(&plain).unwrap();
    let wireless = conduit_host_rp2040::environment_usb_rust(&wireless).unwrap();
    assert!(plain.contains("WIFI_ALLOWED:bool=false"));
    assert!(wireless.contains("WIFI_ALLOWED:bool=true"));
    assert!(wireless.contains("conduit.environment-wifi-carrier@1"));
    assert!(wireless.contains("$p.I2C1,$p.PIN_7,$p.PIN_6"));
}
