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

#[test]
fn checkpoint_connects_the_publication_reference_to_the_existing_body_seed() {
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([12; 32]),
            expires_at: None,
        },
    };
    let checkpoint = RecordingCatalogSeedSource::compile_checkpoint(&reference).unwrap();
    let plot = &checkpoint.plot;
    assert_eq!(plot.expanded.gears.len(), 3);
    assert_eq!(plot.input_bindings.len(), 1);
    assert_eq!(plot.input_bindings[0].front_port_id.as_str(), "catalog");
    let publication = plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND)
        .unwrap();
    let seed = plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND)
        .unwrap();
    let present = plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.gear_id != publication.gear_id && gear.gear_id != seed.gear_id)
        .unwrap();
    assert_eq!(plot.input_bindings[0].gear_id, publication.gear_id);
    assert_eq!(plot.output_bindings[0].gear_id, seed.gear_id);
    assert_eq!(plot.expanded.connections.len(), 2);
    let incoming = |sink: &c::GearId| {
        plot.expanded
            .connections
            .iter()
            .filter(|cord| &cord.sink_gear_id == sink)
            .collect::<Vec<_>>()
    };
    assert_eq!(incoming(&seed.gear_id).len(), 1);
    assert_eq!(incoming(&seed.gear_id)[0].source_gear_id, present.gear_id);
    assert_eq!(incoming(&present.gear_id).len(), 1);
    assert_eq!(
        incoming(&present.gear_id)[0].source_gear_id,
        publication.gear_id
    );
    assert_eq!(
        publication.outputs[0].value_kind.as_str(),
        c::RESOURCE_REFERENCE_INFO_ID
    );
    let c::ConfigurationValue::Text(program) = &present.configuration[0].value else {
        panic!("checked expression")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let encoded = reference.encode().unwrap();
    let reference_hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        publication
            .configuration
            .iter()
            .find(|entry| entry.key == "reference")
            .unwrap()
            .value,
        c::ConfigurationValue::Text(reference_hex.into())
    );
    let result = program.evaluate(&encoded).unwrap();
    let value = c::StructuredInfoValue::from_canonical_bytes(&result).unwrap();
    assert_eq!(value.value_type(), &checkpoint.value_type);
    let c::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("optional reference")
    };
    assert_eq!(tag, "some");
    let c::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("first-class reference")
    };
    assert_eq!(bytes, &encoded);
    assert!(result.len() <= MAXIMUM_CATALOG_SEED_BYTES as usize);
    let read = RecordingCatalogSeedSource::compile_read_only().unwrap();
    assert_eq!(seed.gear_id, read.plot.expanded.gears[0].gear_id);
    assert_eq!(
        seed.configuration,
        read.plot.expanded.gears[0].configuration
    );
    let mut invalid = reference.clone();
    invalid.extent.bytes = 4097;
    assert!(RecordingCatalogSeedSource::compile_checkpoint(&invalid).is_err());
    invalid = reference.clone();
    invalid.content_profile = "value/text".into();
    assert!(RecordingCatalogSeedSource::compile_checkpoint(&invalid).is_err());
    invalid = reference.clone();
    invalid.extent.items = Some(2);
    assert!(RecordingCatalogSeedSource::compile_checkpoint(&invalid).is_err());
}
