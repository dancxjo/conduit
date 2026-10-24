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

#[test]
fn read_only_view_keeps_the_same_state_identity_and_type_without_a_write_fore() {
    let write = RecordingCatalogSeedSource::compile().unwrap();
    let read = RecordingCatalogSeedSource::compile_read_only().unwrap();
    assert_eq!(write.value_type, read.value_type);
    assert_eq!(
        write.plot.expanded.gears[0].gear_id,
        read.plot.expanded.gears[0].gear_id
    );
    assert_eq!(
        write.plot.expanded.gears[0].configuration,
        read.plot.expanded.gears[0].configuration
    );
    assert!(read.plot.input_bindings.is_empty());
    assert_eq!(read.plot.output_bindings.len(), 1);
    assert_eq!(write.plot.output_bindings.len(), 1);
    let read_output = &read.plot.output_bindings[0];
    let write_output = &write.plot.output_bindings[0];
    assert_eq!(read_output.front_port_id, write_output.front_port_id);
    assert_eq!(read_output.gear_id, write_output.gear_id);
    assert_eq!(read_output.gear_port_id, write_output.gear_port_id);
    assert_eq!(read_output.track, write_output.track);
    assert_ne!(
        read.plot.expanded.source_document_id,
        write.plot.expanded.source_document_id
    );
}
