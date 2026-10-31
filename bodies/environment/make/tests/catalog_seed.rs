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

#[test]
fn prepared_checkpoint_retains_valid_catalog_and_refuses_mismatched_extent() {
    use conduit_environment_make::PreparedRecordingCatalogCheckpoint;
    use conduit_environment_runtime::recording::{
        PreparedRecordingCatalog, PreparedRecordingCatalogReader,
    };
    let chunk = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([21; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([22; 32]),
            expires_at: None,
        },
    };
    // Exercise the full overnight catalogue boundary, not just a singleton.
    let chunks: Vec<_> = (0..16)
        .map(|version| {
            let mut reference = chunk.clone();
            reference.lifetime.version = c::ResourceVersionIdentity::from_digest([version + 1; 32]);
            reference
        })
        .collect();
    let catalog = || PreparedRecordingCatalog::prepare(&chunks, 17, 32 * 1024).unwrap();
    let content = catalog().content().to_owned();
    let mut reference = chunk.clone();
    reference.identity = c::ResourceSemanticIdentity::from_digest([23; 32]);
    reference.extent.bytes = content.len() as u64;
    let prepared =
        PreparedRecordingCatalogCheckpoint::prepare(reference.clone(), catalog()).unwrap();
    assert_eq!(prepared.content(), content);
    assert_eq!(prepared.reference(), &reference);
    assert_eq!(prepared.required_records(), 17);
    assert_eq!(
        prepared.required_content_bytes(),
        16 * 1024 + content.len() as u64
    );
    let mut reader = PreparedRecordingCatalogReader::default();
    reader.read(prepared.content()).unwrap();
    assert_eq!(reader.len(), 16);
    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(
            reader.encoded_reference(index).unwrap(),
            chunk.encode().unwrap()
        );
    }
    let expected = RecordingCatalogSeedSource::compile_checkpoint(&reference).unwrap();
    assert_eq!(prepared.source().plot.expanded, expected.plot.expanded);
    for bytes in [0, reference.extent.bytes - 1, reference.extent.bytes + 1] {
        let mut invalid = reference.clone();
        invalid.extent.bytes = bytes;
        assert!(PreparedRecordingCatalogCheckpoint::prepare(invalid, catalog()).is_err());
    }
    let mut invalid = reference;
    invalid.content_profile = "value/text".into();
    assert!(PreparedRecordingCatalogCheckpoint::prepare(invalid, catalog()).is_err());
}

#[test]
fn checkpoint_from_publications_refuses_a_generation_without_kernel_commit() {
    use conduit_environment_runtime::{observation::ObservationContract, recording::*};
    let host = "fixture/producer".into();
    let boot = "fixture/producer-boot".into();
    let plan = "fixture/producer-plan".into();
    let play = "fixture/producer-play".into();
    let context = || RecordingContext {
        timestamp_basis:
            conduit_environment_runtime::recording::RecordingTimestampBasis::Acquisition,
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let contract = ObservationContract::prepare().unwrap();
    let chunk = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([31; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap()
                as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([32; 32]),
            expires_at: None,
        },
    };
    let (_back, observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 2, &chunk).unwrap();
    let catalog =
        PreparedRecordingCatalog::prepare(core::slice::from_ref(&chunk), 2, 8192).unwrap();
    let mut selected = chunk.clone();
    selected.identity = c::ResourceSemanticIdentity::from_digest([33; 32]);
    selected.extent.bytes = catalog.content().len() as u64;
    // Data-only Source preparation is possible, but it is not the Root's
    // acknowledgement-checked publication entrance.
    assert!(
        conduit_environment_make::PreparedRecordingCatalogCheckpoint::prepare(
            selected.clone(),
            catalog
        )
        .is_ok()
    );
    assert!(
        conduit_environment_make::PreparedRecordingCatalogCheckpoint::prepare_after_publications(
            selected,
            &[chunk],
            &[observer],
            2,
            8192,
        )
        .is_err()
    );
}
