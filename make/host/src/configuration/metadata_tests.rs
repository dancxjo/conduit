use crate::test_packages::test_package_set;
use crate::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, ConfigurationDiagnostic, MakeCatalog,
};
const SOURCE: &str =
    include_str!("../../../../targets/rp2040/profiles/pico-w-environment.host.conduit");
fn checked(source: &str) -> crate::CheckedHostConfiguration {
    let packages = test_package_set();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
fn placed(text: &str) -> String {
    SOURCE.replace(
        "  schema =",
        &format!("  metadata = {{placement: {text}}}\n  schema ="),
    )
}
#[test]
fn placement_roundtrips_without_changing_realization_or_legacy_encoding() {
    for source in [
        SOURCE,
        include_str!("../../../../targets/rp2040/profiles/pico-w-bme280-tm1637.host.conduit"),
    ] {
        let old = checked(source);
        assert!(serde_json::to_value(old.configuration())
            .unwrap()
            .get("metadata")
            .is_none());
        let with = source.replace(
            "  schema =",
            "  metadata = {placement: \"Forebrain room\"}\n  schema =",
        );
        let annotated = checked(&with);
        assert_eq!(old.configuration_id(), annotated.configuration_id());
        assert_eq!(old.profile(), annotated.profile());
        assert_eq!(old.resolved_bases(), annotated.resolved_bases());
        let rendered = canonical_host_configuration_conduit(annotated.configuration()).unwrap();
        assert!(rendered.contains("metadata = {placement: \"Forebrain room\"}"));
        let recovered = checked(&rendered);
        assert_eq!(recovered.configuration_id(), annotated.configuration_id());
        assert_eq!(
            recovered.configuration().metadata,
            annotated.configuration().metadata
        );
        assert_eq!(
            canonical_host_configuration_conduit(recovered.configuration()).unwrap(),
            rendered
        );
        let moved = checked(&with.replace("Forebrain room", "Greenhouse"));
        assert_eq!(old.configuration_id(), moved.configuration_id());
        assert_eq!(old.profile(), moved.profile());
    }
}
#[test]
fn placement_is_bounded_utf8_description() {
    for invalid in [
        "\"\"".to_string(),
        "\"   \"".into(),
        serde_json::to_string(&"é".repeat(49)).unwrap(),
        "\"room\\nother\"".into(),
    ] {
        let config = parse_host_configuration_conduit(&placed(&invalid)).unwrap();
        let packages = test_package_set();
        let errors = check_host_configuration(
            config,
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
        .unwrap_err();
        assert!(errors.contains(&ConfigurationDiagnostic::InvalidMetadata));
    }
    let exact = checked(&placed(&serde_json::to_string(&"é".repeat(48)).unwrap()));
    assert_eq!(
        exact
            .configuration()
            .metadata
            .as_ref()
            .unwrap()
            .placement
            .len(),
        96
    );
}
#[test]
fn metadata_unknown_and_duplicate_fields_refuse() {
    for field in [
        "{}",
        "{placement: \"a\", authority: true}",
        "{placement: \"a\", placement: \"b\"}",
    ] {
        let source = SOURCE.replace("  schema =", &format!("  metadata = {field}\n  schema ="));
        assert!(parse_host_configuration_conduit(&source).is_err());
    }
    let duplicate =
        placed("\"a\"").replace("  schema =", "  metadata = {placement: \"b\"}\n  schema =");
    assert!(parse_host_configuration_conduit(&duplicate).is_err());
}
#[test]
fn editable_instrument_locations_preserve_qualified_hardware_pins() {
    for (original, editable, expected) in [
        (
            include_str!("../../../../targets/rp2040/profiles/pico-w-environment-usb.host.conduit"),
            include_str!("../../../../bodies/environment/host/configurations/green.host.conduit"),
            "sha256:7609e0873dbc5481aa45d2a446e0eac32472eb450407a664bb20a00bb2124721",
        ),
        (
            include_str!("../../../../targets/rp2040/profiles/pico-w-bme280-tm1637.host.conduit"),
            include_str!("../../../../bodies/environment/host/configurations/white.host.conduit"),
            "sha256:54bcaf0a60fa1ef6ad289330d0b9f578818030e5e651c6dfb4cdf5a7b1e64449",
        ),
        (
            include_str!("../../../../targets/rp2040/profiles/pico-w-dht11-tm1637.host.conduit"),
            include_str!("../../../../bodies/environment/host/configurations/yellow.host.conduit"),
            "sha256:5ac1c36771db2cd151fc9c8985463026ed88ec4c1633e457d91bacd732dece9c",
        ),
    ] {
        let old = checked(original);
        let current = checked(editable);
        assert_eq!(old.configuration_id(), expected);
        assert_eq!(current.configuration_id(), expected);
        assert_eq!(old.profile(), current.profile());
        let mut stripped = current.configuration().clone();
        stripped.metadata = None;
        assert_eq!(old.configuration(), &stripped);
    }
}
