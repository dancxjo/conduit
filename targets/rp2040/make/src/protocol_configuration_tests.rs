//! Checked physical facts and actual Source laws; no device/Play claim.
use super::*;
use conduit_core::{validate_canonical_structured_value, StructuredInfoType, StructuredInfoValue};
use conduit_host_make::{
    check_host_configuration, parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
const GREEN: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
fn host(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&crate::Rp2040MakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
}
fn source() -> CheckedSyntaxDocument {
    let mut startup = StartupCatalog::new();
    conduit_mcu_source_build::protocol_catalog::install(&mut startup, &mut ProfileCatalog::new())
        .unwrap();
    let units = [
        include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33.conduit"),
    ];
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|s| s.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let body = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|s| !s.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    check_syntax_document(
        &parse_syntax_document(&format!("{imports}\n{body}")),
        &startup,
    )
    .unwrap()
}
fn field(image: &LinkedSourceImage, name: &str) -> u8 {
    let value = validate_canonical_structured_value(image.value()).unwrap();
    let leaf = value.record_field(name).unwrap().unwrap();
    *leaf.value_node().last().unwrap()
}
#[test]
fn facts_follow_checked_host_and_preserve_exact_source_schema() {
    let source = source();
    let original =
        CheckedProtocolConfigurationImages::prepare(&host(GREEN), &source, 0, true).unwrap();
    assert_eq!(field(&original.sensor, "address"), 118);
    assert_eq!(field(&original.display, "address"), 112);
    assert_eq!(field(&original.display, "brightness"), 3);
    assert_eq!(field(&original.display, "blink"), 0);
    assert_eq!(field(&original.display, "enabled"), 1);
    assert_eq!(original.source_document_id, source.source_document_id);
    for (image, name) in [
        (&original.sensor, "BmeProtocolBegin"),
        (&original.display, "Ht16k33Configuration"),
    ] {
        let native = source.native_types.iter().find(|n| n.name == name).unwrap();
        assert_eq!(
            StructuredInfoType::from_canonical_bytes(image.schema()).unwrap(),
            native.value_type
        );
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(image.value())
                .unwrap()
                .value_type(),
            &native.value_type
        );
        assert!(image.value().len() <= image.maximum_bytes() as usize);
        assert!(image.rust_definition().contains("LinkedSourceStatic"));
    }
    let changed = CheckedProtocolConfigurationImages::prepare(
        &host(&GREEN.replace("brightness: 3", "brightness: 9")),
        &source,
        2,
        false,
    )
    .unwrap();
    assert_ne!(changed.configuration_id, original.configuration_id);
    assert_eq!(changed.sensor.value(), original.sensor.value());
    assert_eq!(field(&changed.display, "brightness"), 9);
    assert_eq!(field(&changed.display, "blink"), 2);
    assert_eq!(field(&changed.display, "enabled"), 0);
}
#[test]
fn original_native_refinement_and_type_remain_preparation_gates() {
    let checked = host(GREEN);
    let mut source = source();
    assert!(CheckedProtocolConfigurationImages::prepare(&checked, &source, 4, true).is_err());
    let sensor = source
        .native_types
        .iter_mut()
        .find(|n| n.name == "BmeProtocolBegin")
        .unwrap();
    // An independently checked narrower Source excludes the installed address.
    let narrow = check_syntax_document(
        &parse_syntax_document("type BmeProtocolBegin = {\n address: U8 in 8..=117\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    *sensor = narrow.native_types[0].clone();
    assert!(
        matches!(CheckedProtocolConfigurationImages::prepare(&checked, &source, 0, true), Err(e) if e.contains("Native laws"))
    );
}
