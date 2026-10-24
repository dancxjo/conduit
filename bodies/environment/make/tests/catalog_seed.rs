use conduit_core as c;
use conduit_environment_make::{RecordingCatalogSeedSource, MAXIMUM_CATALOG_SEED_BYTES};
#[test]
fn catalog_seed_is_checked_body_lived_optional_resource_reference() {
    let source = RecordingCatalogSeedSource::compile().unwrap();
    let [seed] = source.plot.expanded.gears.as_slice() else {
        panic!("one ordinary State")
    };
    assert_eq!(seed.kind_id.as_str(), c::STATE_VALUE_KIND);
    let configuration = |key| {
        &seed
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .unwrap()
            .value
    };
    assert_eq!(
        configuration("retained-duration"),
        &c::ConfigurationValue::Text("body".into())
    );
    assert_eq!(
        configuration("maximum-bytes"),
        &c::ConfigurationValue::U64(u64::from(MAXIMUM_CATALOG_SEED_BYTES))
    );
    let c::ConfigurationValue::Structured(initial) = configuration("initial") else {
        panic!("typed seed")
    };
    let value = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    assert_eq!(value.value_type(), &source.value_type);
    assert!(matches!(
        value.shape(),
        c::StructuredInfoValueShape::Variant { tag: "none", .. }
    ));
    let expected = source.value_type.profile().unwrap().value_kind().clone();
    assert_eq!(seed.inputs[0].value_kind, expected);
    assert_eq!(seed.outputs[0].value_kind, expected);
    assert_eq!(seed.inputs[0].temporal, c::PortTemporal::Value);
    assert_eq!(seed.outputs[0].temporal, c::PortTemporal::Current);
}
