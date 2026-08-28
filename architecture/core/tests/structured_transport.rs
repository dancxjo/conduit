use conduit_core::{
    decode_structured_transport, encode_structured_transport, KindId, StructuredFieldType,
    StructuredFieldValue, StructuredInfoTransportRefusal, StructuredInfoType, StructuredInfoValue,
    StructuredVariantCase, MAXIMUM_STRUCTURED_TRANSPORT_BYTES,
};

fn leaf_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from(kind)).unwrap()
}

fn leaf(kind: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf_type(kind), bytes.to_vec()).unwrap()
}

fn music_type() -> StructuredInfoType {
    let note = StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![
            StructuredFieldType::new("pitch", leaf_type("value/count@1")).unwrap(),
            StructuredFieldType::new("velocity", leaf_type("value/count@1")).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![StructuredVariantCase::new("note", note).unwrap()],
    )
    .unwrap()
}

fn music_value() -> StructuredInfoValue {
    let note_type = match music_type().shape() {
        conduit_core::StructuredInfoTypeShape::Variant { cases, .. } => {
            cases[0].payload_type().clone()
        }
        _ => unreachable!(),
    };
    let note = StructuredInfoValue::record(
        note_type,
        vec![
            StructuredFieldValue::new("velocity", leaf("value/count@1", &[96])).unwrap(),
            StructuredFieldValue::new("pitch", leaf("value/count@1", &[60])).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(music_type(), "note", note).unwrap()
}

fn llm_type() -> StructuredInfoType {
    StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![
            StructuredFieldType::new("confidence", leaf_type("value/count@1")).unwrap(),
            StructuredFieldType::new("text", leaf_type("value/text@1")).unwrap(),
        ],
    )
    .unwrap()
}

fn llm_value() -> StructuredInfoValue {
    StructuredInfoValue::record(
        llm_type(),
        vec![
            StructuredFieldValue::new("text", leaf("value/text@1", b"bounded answer")).unwrap(),
            StructuredFieldValue::new("confidence", leaf("value/count@1", &[91])).unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn music_and_llm_values_share_one_bounded_versioned_transport() {
    for value in [music_value(), llm_value()] {
        let encoded = encode_structured_transport(
            &value,
            u32::try_from(MAXIMUM_STRUCTURED_TRANSPORT_BYTES).unwrap(),
        )
        .unwrap();
        assert_eq!(
            decode_structured_transport(
                value.value_type(),
                &encoded,
                u32::try_from(encoded.len()).unwrap(),
            )
            .unwrap(),
            value
        );
    }
}

#[test]
fn semantic_profile_is_shape_derived_and_not_authored_alias_text() {
    let first = llm_type();
    let reordered = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![
            StructuredFieldType::new("text", leaf_type("value/text@1")).unwrap(),
            StructuredFieldType::new("confidence", leaf_type("value/count@1")).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(first.profile().unwrap(), reordered.profile().unwrap());
    assert!(first
        .profile()
        .unwrap()
        .value_kind()
        .as_str()
        .starts_with("structured-info/profile-"));
    assert_ne!(first.profile().unwrap(), music_type().profile().unwrap());
}

#[test]
fn size_version_profile_and_mutation_fail_distinctly() {
    let value = music_value();
    let encoded = encode_structured_transport(
        &value,
        u32::try_from(MAXIMUM_STRUCTURED_TRANSPORT_BYTES).unwrap(),
    )
    .unwrap();
    assert_eq!(
        encode_structured_transport(&value, u32::try_from(encoded.len() - 1).unwrap()),
        Err(StructuredInfoTransportRefusal::SizeExhausted)
    );
    assert_eq!(
        decode_structured_transport(
            &value.value_type().clone(),
            &encoded,
            u32::try_from(encoded.len() - 1).unwrap(),
        ),
        Err(StructuredInfoTransportRefusal::SizeExhausted)
    );

    let mut wrong_version = encoded.clone();
    wrong_version[6] = 2;
    assert_eq!(
        decode_structured_transport(
            value.value_type(),
            &wrong_version,
            u32::try_from(wrong_version.len()).unwrap(),
        ),
        Err(StructuredInfoTransportRefusal::UnsupportedVersion)
    );
    assert_eq!(
        decode_structured_transport(&llm_type(), &encoded, u32::try_from(encoded.len()).unwrap(),),
        Err(StructuredInfoTransportRefusal::ProfileMismatch)
    );

    let mut mutated = encoded;
    *mutated.last_mut().unwrap() ^= 1;
    assert_eq!(
        decode_structured_transport(
            value.value_type(),
            &mutated,
            u32::try_from(mutated.len()).unwrap(),
        ),
        Err(StructuredInfoTransportRefusal::ValueIdentityMismatch)
    );
}

#[test]
fn malformed_lengths_tags_names_and_trailing_bytes_never_decode() {
    let value = llm_value();
    let encoded = encode_structured_transport(
        &value,
        u32::try_from(MAXIMUM_STRUCTURED_TRANSPORT_BYTES).unwrap(),
    )
    .unwrap();
    for malformed in [
        encoded[..encoded.len() - 1].to_vec(),
        {
            let mut value = encoded.clone();
            value.extend_from_slice(&[0]);
            value
        },
        {
            let mut value = encoded.clone();
            value[0] = b'X';
            value
        },
    ] {
        assert_eq!(
            decode_structured_transport(
                value.value_type(),
                &malformed,
                u32::try_from(MAXIMUM_STRUCTURED_TRANSPORT_BYTES).unwrap(),
            ),
            Err(StructuredInfoTransportRefusal::MalformedRepresentation)
        );
    }
}
