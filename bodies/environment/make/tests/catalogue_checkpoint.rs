//! #6359 authored Source and pure constructor only. No installed effects, prior
//! chunk commits, producer ACK, Body admission or fresh-process recovery claim.
use catalogue::{PreparedRecordingCatalog, PreparedRecordingCatalogReader};
use conduit_core as c;
#[path = "catalogue_checkpoint/catalogue.rs"]
mod catalogue;
#[path = "catalogue_checkpoint/source.rs"]
mod source;

fn reference(seed: u8, bytes: u64) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([seed; 32]),
            expires_at: None,
        },
    }
}
fn validated_catalogue() -> (PreparedRecordingCatalog, c::BoundedResourceRef) {
    let chunks = (1..=16)
        .map(|seed| reference(seed, 1024))
        .collect::<Vec<_>>();
    let catalogue = PreparedRecordingCatalog::prepare(&chunks, 17, 32 * 1024).unwrap();
    // Validation is independent of compilation/input construction. Neither this
    // catalogue nor its references assert that the listed chunks were published.
    let mut reader = PreparedRecordingCatalogReader::default();
    reader.read(catalogue.content()).unwrap();
    assert!(!reader.is_empty());
    assert_eq!(reader.len(), 16);
    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(
            reader.encoded_reference(index),
            Some(chunk.encode().unwrap().as_slice())
        );
    }
    let selected = reference(99, catalogue.content().len() as u64);
    (catalogue, selected)
}

#[test]
fn validated_catalogue_has_the_preserved_exact_schema_and_finite_envelope() {
    let (catalogue, _) = validated_catalogue();
    let expected = format!(
        "{{\"schema\":\"conduit.environment/recording-catalog@1\",\"references\":[{}]}}",
        (1..=16)
            .map(|seed| {
                let hex = reference(seed, 1024)
                    .encode()
                    .unwrap()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!("\"{hex}\"")
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(catalogue.content(), expected);
    assert!(catalogue.content().len() <= 4096);
    assert_eq!(catalogue.required_records(), 17);
    assert_eq!(
        catalogue.required_content_bytes(),
        16 * 1024 + expected.len() as u64
    );
}

#[test]
fn sole_json_input_reaches_seed_only_through_publication_and_exact_pure_some() {
    let (catalogue, reference) = validated_catalogue();
    assert_eq!(reference.extent.bytes, catalogue.content().len() as u64);
    let checkpoint = source::checkpoint(&reference).unwrap();
    let plot = &checkpoint.plot;
    assert_eq!(plot.expanded.gears.len(), 3);
    assert_eq!(plot.input_bindings.len(), 1);
    assert_eq!(plot.input_bindings[0].front_port_id.as_str(), "catalog");
    assert_eq!(plot.output_bindings.len(), 1);
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
    assert_eq!(
        plot.input_bindings[0].gear_port_id,
        publication.inputs[0].port_id
    );
    assert_eq!(plot.input_bindings[0].track, c::ConnectionTrack::Payload);
    assert_eq!(
        publication.inputs[0].value_kind.as_str(),
        conduit_web::JSON_TEXT_INFO_ID
    );
    assert_eq!(plot.output_bindings[0].gear_id, seed.gear_id);
    assert_eq!(plot.expanded.connections.len(), 2);
    for (from, to) in [(publication, present), (present, seed)] {
        let incoming = plot
            .expanded
            .connections
            .iter()
            .filter(|cord| cord.sink_gear_id == to.gear_id)
            .collect::<Vec<_>>();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source_gear_id, from.gear_id);
        assert_eq!(incoming[0].source_port_id, from.outputs[0].port_id);
        assert_eq!(incoming[0].sink_port_id, to.inputs[0].port_id);
        assert_eq!(incoming[0].value_kind, from.outputs[0].value_kind);
        assert_eq!(incoming[0].value_kind, to.inputs[0].value_kind);
        assert_eq!(incoming[0].track, c::ConnectionTrack::Payload);
    }
    assert_eq!(
        publication.outputs[0].value_kind.as_str(),
        c::RESOURCE_REFERENCE_INFO_ID
    );
    let c::ConfigurationValue::Text(program) = &present
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("pure Some program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let encoded = reference.encode().unwrap();
    let result = program.evaluate(&encoded).unwrap();
    let value = c::StructuredInfoValue::from_canonical_bytes(&result).unwrap();
    assert_eq!(value.value_type(), &checkpoint.value_type);
    let c::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("optional reference")
    };
    assert_eq!(tag, "some");
    let c::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("reference atom")
    };
    assert_eq!(bytes, &encoded);
    assert!(result.len() <= source::MAXIMUM_SEED_BYTES as usize);
    assert!(program.evaluate(b"not a canonical reference").is_err());
}

#[test]
fn checkpoint_and_read_only_preserve_exact_body_state_identity_type_and_configuration() {
    let (_, reference) = validated_catalogue();
    let write = source::checkpoint(&reference).unwrap();
    let read = source::read_only().unwrap();
    let seed = write
        .plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND)
        .unwrap();
    let [read_seed] = read.plot.expanded.gears.as_slice() else {
        panic!("one read-only State")
    };
    assert_eq!(seed.gear_id, read_seed.gear_id); // Planner derives StateId from this exact GearId.
    assert_eq!(seed.configuration, read_seed.configuration);
    assert_eq!(write.value_type, read.value_type);
    assert!(read.plot.input_bindings.is_empty());
    assert_eq!(read.plot.output_bindings.len(), 1);
    assert_eq!(
        write.plot.output_bindings[0].gear_id,
        read.plot.output_bindings[0].gear_id
    );
    let config = |key| {
        &seed
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .unwrap()
            .value
    };
    assert_eq!(
        config("retained-duration"),
        &c::ConfigurationValue::Text("body".into())
    );
    assert_eq!(config("maximum-bytes"), &c::ConfigurationValue::U64(1024));
    let c::ConfigurationValue::Structured(initial) = config("initial") else {
        panic!("typed initial")
    };
    let initial = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    assert_eq!(initial.value_type(), &read.value_type);
    assert!(matches!(
        initial.shape(),
        c::StructuredInfoValueShape::Variant { tag: "none", .. }
    ));
    assert_eq!(seed.inputs[0].temporal, c::PortTemporal::Value);
    assert_eq!(seed.outputs[0].temporal, c::PortTemporal::Current);
}

#[test]
fn checkpoint_refuses_foreign_profiles_and_unbounded_or_expiring_selection() {
    let (_, selected) = validated_catalogue();
    for mutate in [
        (|r: &mut c::BoundedResourceRef| r.content_profile = "value/text".into())
            as fn(&mut c::BoundedResourceRef),
        |r| r.access_class = "foreign/access".into(),
        |r| r.extent.bytes = 0,
        |r| r.extent.bytes = 4097,
        |r| r.extent.items = None,
        |r| r.extent.items = Some(2),
        |r| {
            r.lifetime.expires_at = Some(c::TemporalInstant {
                ticks: 1,
                scale: c::TemporalScale::Milliseconds,
                clock_basis: "fixture/clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            })
        },
    ] {
        let mut changed = selected.clone();
        mutate(&mut changed);
        assert!(source::checkpoint(&changed).is_err());
    }
}

#[test]
fn full_catalogue_matches_whole_json_text_without_tree_profile_admission() {
    let (catalogue, _) = validated_catalogue();
    assert_eq!(
        conduit_web::validate_json_text(catalogue.content().as_bytes()),
        Ok(())
    );
    // A bounded JSON tree is a different contract from the original text. The
    // selected JSON snapshot publisher validates whole text without building it.
    assert_eq!(
        conduit_web::JsonValue::decode_text(catalogue.content().as_bytes()),
        Err(conduit_web::JsonRefusal::TotalStringByteOverflow)
    );
}
