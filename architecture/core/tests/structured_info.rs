use conduit_core::{
    encode_count, optional_info_type, validate_canonical_structured_value, KindId,
    PreparedOptionalInfoEncoder, RuntimeStructuredInfo, Scalar, StartupStructuredValue,
    StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase, MAXIMUM_STRUCTURED_COLLECTION_ITEMS,
    MAXIMUM_STRUCTURED_INFO_DEPTH, MAXIMUM_STRUCTURED_LEAF_BYTES,
};

#[test]
fn optional_info_is_exactly_the_finite_none_or_some_variant() {
    let scalar = leaf_type("value/scalar");
    let optional = optional_info_type(scalar.clone()).unwrap();
    let none =
        StructuredInfoValue::variant(optional.clone(), "none", leaf("value/unit", &[])).unwrap();
    let some = StructuredInfoValue::variant(
        optional.clone(),
        "some",
        StructuredInfoValue::leaf(scalar, Scalar::from_raw_microunits(7).encode().to_vec())
            .unwrap(),
    )
    .unwrap();

    assert_ne!(
        none.canonical_bytes().unwrap(),
        some.canonical_bytes().unwrap()
    );
    assert_eq!(
        StructuredInfoValue::variant(optional, "none", leaf("value/scalar", &[0; 8])),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn prepared_optional_encoding_matches_the_canonical_variant_without_growth() {
    let scalar = leaf_type("value/scalar");
    let optional = optional_info_type(scalar.clone()).unwrap();
    let scalar_bytes = Scalar::from_raw_microunits(7).encode();
    let expected_some = StructuredInfoValue::variant(
        optional.clone(),
        "some",
        StructuredInfoValue::leaf(scalar.clone(), scalar_bytes.to_vec()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let expected_none = StructuredInfoValue::variant(optional, "none", leaf("value/unit", &[]))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut encoder = PreparedOptionalInfoEncoder::new(scalar).unwrap();
    let capacity = encoder.capacity();
    assert_eq!(encoder.encode(Some(&scalar_bytes)).unwrap(), expected_some);
    assert_eq!(encoder.capacity(), capacity);
    assert_eq!(encoder.encode(None).unwrap(), expected_none);
    assert_eq!(encoder.capacity(), capacity);
}

fn leaf_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from(kind)).unwrap()
}

fn leaf(kind: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf_type(kind), bytes.to_vec()).unwrap()
}

#[test]
fn record_field_order_is_canonically_irrelevant_and_digest_stable() {
    let pitch = StructuredFieldType::new("pitch", leaf_type("sound/pitch@1")).unwrap();
    let velocity = StructuredFieldType::new("velocity", leaf_type("value/scalar")).unwrap();
    let first = StructuredInfoType::record(
        KindId::from("music/note-event@1"),
        vec![pitch.clone(), velocity.clone()],
    )
    .unwrap();
    let second =
        StructuredInfoType::record(KindId::from("music/note-event@1"), vec![velocity, pitch])
            .unwrap();

    assert_eq!(first, second);
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert_eq!(
        first.semantic_digest().unwrap(),
        second.semantic_digest().unwrap()
    );

    let first_value = StructuredInfoValue::record(
        first.clone(),
        vec![
            StructuredFieldValue::new(
                "velocity",
                leaf("value/scalar", &Scalar::from_raw_microunits(80).encode()),
            )
            .unwrap(),
            StructuredFieldValue::new("pitch", leaf("sound/pitch@1", &[60])).unwrap(),
        ],
    )
    .unwrap();
    let second_value = StructuredInfoValue::record(
        second,
        vec![
            StructuredFieldValue::new("pitch", leaf("sound/pitch@1", &[60])).unwrap(),
            StructuredFieldValue::new(
                "velocity",
                leaf("value/scalar", &Scalar::from_raw_microunits(80).encode()),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(first_value, second_value);
    assert_eq!(
        first_value.semantic_digest().unwrap(),
        second_value.semantic_digest().unwrap()
    );
}

#[test]
fn nominal_schemas_prevent_protocol_and_portable_records_from_accidental_aliasing() {
    let fields = vec![
        StructuredFieldType::new("pitch", leaf_type("value/count")).unwrap(),
        StructuredFieldType::new("velocity", leaf_type("value/count")).unwrap(),
    ];
    let midi = StructuredInfoType::record(KindId::from("midi/note-on@1"), fields.clone()).unwrap();
    let portable = StructuredInfoType::record(KindId::from("music/note-on@1"), fields).unwrap();

    assert_ne!(midi, portable);
    assert_ne!(
        midi.semantic_digest().unwrap(),
        portable.semantic_digest().unwrap()
    );
}

#[test]
fn nominal_types_keep_distinct_identity_over_the_same_exact_representation() {
    let representation = leaf_type("value/count");
    let note =
        StructuredInfoType::nominal(KindId::from("music/note@1"), representation.clone()).unwrap();
    let velocity =
        StructuredInfoType::nominal(KindId::from("music/velocity@1"), representation.clone())
            .unwrap();
    let represented = StructuredInfoValue::leaf(representation, encode_count(64).to_vec()).unwrap();
    let note_value = StructuredInfoValue::nominal(note.clone(), represented.clone()).unwrap();
    let velocity_value = StructuredInfoValue::nominal(velocity.clone(), represented).unwrap();

    assert_ne!(note, velocity);
    assert_ne!(note.profile().unwrap(), velocity.profile().unwrap());
    assert_ne!(
        note_value.semantic_digest().unwrap(),
        velocity_value.semantic_digest().unwrap()
    );
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&note_value.canonical_bytes().unwrap()),
        Ok(note_value)
    );
    assert_eq!(
        StructuredInfoValue::nominal(note, velocity_value),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn variants_keep_exact_tags_and_validate_payload_types() {
    let event = StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![
            StructuredVariantCase::new("note_off", leaf_type("music/note-off@1")).unwrap(),
            StructuredVariantCase::new("note_on", leaf_type("music/note-on@1")).unwrap(),
        ],
    )
    .unwrap();
    let on = StructuredInfoValue::variant(event.clone(), "note_on", leaf("music/note-on@1", &[1]))
        .unwrap();
    let off =
        StructuredInfoValue::variant(event.clone(), "note_off", leaf("music/note-off@1", &[1]))
            .unwrap();

    assert_ne!(
        on.semantic_digest().unwrap(),
        off.semantic_digest().unwrap()
    );
    assert_eq!(
        StructuredInfoValue::variant(event.clone(), "control", leaf("music/control@1", &[1])),
        Err(StructuredInfoRefusal::UnknownVariantTag)
    );
    assert_eq!(
        StructuredInfoValue::variant(event, "note_on", leaf("music/note-off@1", &[1])),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn nested_finite_instrument_and_feedback_shapes_share_one_substrate() {
    let pitch_table_type =
        StructuredInfoType::collection(leaf_type("music/pitch@1"), Some(9)).unwrap();
    let pitches = (0..9)
        .map(|pitch| leaf("music/pitch@1", &[pitch]))
        .collect();
    let pitch_table = StructuredInfoValue::collection(pitch_table_type.clone(), pitches).unwrap();

    let feedback_type = StructuredInfoType::record(
        KindId::from("education/rhythm-feedback@1"),
        vec![
            StructuredFieldType::new("classification", leaf_type("education/beat-class@1"))
                .unwrap(),
            StructuredFieldType::new("delta", leaf_type("time/duration@1")).unwrap(),
            StructuredFieldType::new("expected", leaf_type("time/instant@1")).unwrap(),
            StructuredFieldType::new("observed", leaf_type("time/instant@1")).unwrap(),
        ],
    )
    .unwrap();
    let feedback = StructuredInfoValue::record(
        feedback_type,
        vec![
            StructuredFieldValue::new("observed", leaf("time/instant@1", &[2])).unwrap(),
            StructuredFieldValue::new("expected", leaf("time/instant@1", &[1])).unwrap(),
            StructuredFieldValue::new("delta", leaf("time/duration@1", &[1])).unwrap(),
            StructuredFieldValue::new("classification", leaf("education/beat-class@1", b"late"))
                .unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(pitch_table.value_type(), &pitch_table_type);
    assert_ne!(
        pitch_table.semantic_digest().unwrap(),
        feedback.semantic_digest().unwrap()
    );
}

#[test]
fn borrowed_node_validation_checks_nested_shape_without_reconstruction() {
    let item_type = StructuredInfoType::variant(
        KindId::from("test/item@1"),
        vec![StructuredVariantCase::new("some", leaf_type("value/text")).unwrap()],
    )
    .unwrap();
    let collection_type = StructuredInfoType::collection(item_type.clone(), Some(1)).unwrap();
    let value = StructuredInfoValue::collection(
        collection_type.clone(),
        vec![
            StructuredInfoValue::variant(item_type, "some", leaf("value/text", b"bounded"))
                .unwrap(),
        ],
    )
    .unwrap();
    let canonical = value.canonical_bytes().unwrap();
    let type_bytes = collection_type.canonical_bytes().unwrap();
    let node = canonical.strip_prefix(type_bytes.as_slice()).unwrap();
    assert_eq!(collection_type.validate_canonical_node(node), Ok(()));
    let validated = validate_canonical_structured_value(&canonical).unwrap();
    assert_eq!(validated.type_bytes(), type_bytes);
    assert_eq!(validated.value_node(), node);
    let mut malformed = node.to_vec();
    malformed.push(0);
    assert_eq!(
        collection_type.validate_canonical_node(&malformed),
        Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
    );
}

#[test]
fn exact_structured_type_round_trips_without_a_value_node() {
    let value_type = StructuredInfoType::record(
        KindId::from("test/type-round-trip@1"),
        vec![StructuredFieldType::new("value", leaf_type("value/count")).unwrap()],
    )
    .unwrap();
    let encoded = value_type.canonical_bytes().unwrap();
    assert_eq!(
        StructuredInfoType::from_canonical_bytes(&encoded),
        Ok(value_type)
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        StructuredInfoType::from_canonical_bytes(&trailing),
        Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
    );
}

#[test]
fn bounded_sequence_preserves_element_type_and_canonical_actual_length() {
    let element = leaf_type("value/count");
    assert_eq!(
        StructuredInfoType::bounded_sequence(element.clone(), 3, 2),
        Err(StructuredInfoRefusal::InvalidCollectionBounds)
    );
    let sequence_type = StructuredInfoType::bounded_sequence(element.clone(), 2, 4).unwrap();
    let optional_sequence = StructuredInfoType::sequence(element.clone(), 4).unwrap();
    assert_ne!(
        sequence_type.semantic_digest().unwrap(),
        optional_sequence.semantic_digest().unwrap()
    );
    assert_ne!(
        sequence_type.canonical_bytes().unwrap(),
        optional_sequence.canonical_bytes().unwrap()
    );
    StructuredInfoValue::sequence(optional_sequence, vec![]).unwrap();
    let values = vec![
        leaf("value/count", &encode_count(1)),
        leaf("value/count", &encode_count(2)),
    ];
    let sequence = StructuredInfoValue::sequence(sequence_type.clone(), values).unwrap();
    let canonical = sequence.canonical_bytes().unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&canonical),
        Ok(sequence)
    );
    assert!(validate_canonical_structured_value(&canonical).is_ok());
    assert_eq!(
        StructuredInfoValue::sequence(sequence_type.clone(), vec![]),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );
    assert_eq!(
        StructuredInfoValue::sequence(
            sequence_type.clone(),
            vec![leaf("value/count", &encode_count(1))]
        ),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );
    assert_eq!(
        StructuredInfoValue::sequence(
            sequence_type.clone(),
            (0..5)
                .map(|value| leaf("value/count", &encode_count(value)))
                .collect(),
        ),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );
    assert_eq!(
        StructuredInfoValue::sequence(
            sequence_type,
            vec![
                leaf("value/text", b"wrong element"),
                leaf("value/text", b"wrong element"),
            ],
        ),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn validated_llm_extraction_uses_the_same_nominal_record_model() {
    let extraction_type = StructuredInfoType::record(
        KindId::from("education/lesson-extraction@1"),
        vec![
            StructuredFieldType::new("confidence", leaf_type("value/scalar")).unwrap(),
            StructuredFieldType::new("topic", leaf_type("value/text")).unwrap(),
        ],
    )
    .unwrap();
    let extraction = StructuredInfoValue::record(
        extraction_type.clone(),
        vec![
            StructuredFieldValue::new("topic", leaf("value/text", b"rhythm")).unwrap(),
            StructuredFieldValue::new(
                "confidence",
                leaf("value/scalar", &Scalar::from_raw_microunits(75).encode()),
            )
            .unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(extraction.value_type(), &extraction_type);
    assert!(extraction.semantic_digest().is_ok());
}

#[test]
fn bounds_and_dynamic_shapes_fail_closed() {
    assert_eq!(
        StructuredInfoType::collection(leaf_type("value/count"), None),
        Err(StructuredInfoRefusal::UnboundedCollection)
    );
    assert_eq!(
        StructuredInfoType::collection(
            leaf_type("value/count"),
            Some((MAXIMUM_STRUCTURED_COLLECTION_ITEMS + 1) as u16),
        ),
        Err(StructuredInfoRefusal::CollectionTooLarge)
    );
    assert_eq!(
        StructuredInfoValue::leaf(
            leaf_type("value/blob@1"),
            vec![0; MAXIMUM_STRUCTURED_LEAF_BYTES + 1],
        ),
        Err(StructuredInfoRefusal::LeafTooLarge)
    );

    let mut nested = leaf_type("value/count");
    for _ in 1..MAXIMUM_STRUCTURED_INFO_DEPTH {
        nested = StructuredInfoType::collection(nested, Some(1)).unwrap();
    }
    assert_eq!(
        StructuredInfoType::collection(nested, Some(1)),
        Err(StructuredInfoRefusal::TooDeep)
    );
}

#[test]
fn exact_collection_lengths_and_record_members_are_checked() {
    let pair = StructuredInfoType::collection(leaf_type("value/count"), Some(2)).unwrap();
    assert_eq!(
        StructuredInfoValue::collection(pair, vec![leaf("value/count", &encode_count(1))]),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );

    let record = StructuredInfoType::record(
        KindId::from("test/point@1"),
        vec![StructuredFieldType::new("x", leaf_type("value/scalar")).unwrap()],
    )
    .unwrap();
    assert_eq!(
        StructuredInfoValue::record(
            record,
            vec![
                StructuredFieldValue::new("y", leaf("value/scalar", &Scalar::ZERO.encode()),)
                    .unwrap()
            ],
        ),
        Err(StructuredInfoRefusal::WrongRecordFields)
    );
}

#[test]
fn startup_and_runtime_contexts_remain_distinct_types() {
    let value = leaf("value/count", &encode_count(7));
    let startup = StartupStructuredValue::new(value.clone());
    let runtime = RuntimeStructuredInfo::new(value);

    assert_eq!(startup.value(), runtime.value());
    assert_eq!(
        core::any::type_name_of_val(&startup),
        "conduit_core::structured_info::StartupStructuredValue"
    );
    assert_eq!(
        core::any::type_name_of_val(&runtime),
        "conduit_core::structured_info::RuntimeStructuredInfo"
    );
}
