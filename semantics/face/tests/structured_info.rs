use conduit_core::{
    BootId, HostId, KindId, Observation, ObservationKind, Quantity, SignId, StructuredFieldType,
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredVariantCase, Unit,
    ValuePayload,
};
use conduit_face::{
    FaceAspect, FaceCursor, FaceDepth, FacePlace, FacePropertyValue, StructuredSignFace,
};

fn leaf_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from(kind)).unwrap()
}

fn leaf(kind: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf_type(kind), bytes.to_vec()).unwrap()
}

fn sign(value: &StructuredInfoValue) -> Observation {
    Observation {
        sign_id: SignId::from("sign/presented-structured"),
        active_play_id: None,
        face_id: None,
        host_id: HostId::from("host/local"),
        boot_id: BootId::from("boot/1"),
        plan_id: None,
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::ValuePresented {
            label: "Observation".into(),
            value: ValuePayload {
                value_kind: value.value_type().profile().unwrap().value_kind().clone(),
                encoded: value.canonical_bytes().unwrap(),
            },
        },
    }
}

fn project_at(artifact: &StructuredSignFace, depth: FaceDepth) -> usize {
    artifact
        .projection
        .project(
            &artifact.face,
            &artifact.navigation,
            &FaceCursor {
                face: artifact.face.identity.clone(),
                navigation: artifact.navigation.identity.clone(),
                revision: artifact.face.revision,
                place: FacePlace::Body,
                aspect: FaceAspect::Signs,
                focus: None,
                depth,
            },
        )
        .unwrap()
        .items
        .len()
}

#[test]
fn structured_sign_projection_discloses_shape_progressively_without_leaf_content() {
    let note_type = StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![StructuredFieldType::new("pitch", leaf_type("value/count")).unwrap()],
    )
    .unwrap();
    let event_type = StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![StructuredVariantCase::new("note_on", note_type.clone()).unwrap()],
    )
    .unwrap();
    let note = StructuredInfoValue::record(
        note_type,
        vec![StructuredFieldValue::new(
            "pitch",
            leaf("value/count", &conduit_core::encode_count(60)),
        )
        .unwrap()],
    )
    .unwrap();
    let event = StructuredInfoValue::variant(event_type.clone(), "note_on", note).unwrap();
    let artifact = StructuredSignFace::from_sign(4, &sign(&event), &event_type).unwrap();

    let primary = project_at(&artifact, FaceDepth::Primary);
    let context = project_at(&artifact, FaceDepth::Context);
    let detail = project_at(&artifact, FaceDepth::Detail);
    assert!(primary < context && context < detail);
    assert_eq!(
        artifact.face.basis.sign_ids,
        vec![SignId::from("sign/presented-structured")]
    );
    assert!(artifact.face.properties.iter().any(|property| {
        property.name == "active-variant-tag"
            && property.value == FacePropertyValue::Identity("note_on".into())
    }));
}

#[test]
fn llm_leaf_bytes_never_enter_face_content() {
    let output_type = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![StructuredFieldType::new("answer", leaf_type("value/text")).unwrap()],
    )
    .unwrap();
    let output = StructuredInfoValue::record(
        output_type.clone(),
        vec![
            StructuredFieldValue::new("answer", leaf("value/text", b"private model output"))
                .unwrap(),
        ],
    )
    .unwrap();
    let artifact = StructuredSignFace::from_sign(1, &sign(&output), &output_type).unwrap();

    assert!(!format!("{:?}", artifact.face).contains("private model output"));
    assert!(artifact.face.properties.iter().any(|property| {
        property.name == "leaf-content-redacted" && property.value == FacePropertyValue::Flag(true)
    }));
}

#[test]
fn quantity_sign_projects_unit_identity_and_signed_value_without_formatting_text() {
    let quantity = Quantity::new(-17, Unit::Millivolt);
    let value = leaf(conduit_core::QUANTITY_INFO_ID, &quantity.encode());
    let artifact = StructuredSignFace::from_sign(2, &sign(&value), value.value_type()).unwrap();

    assert!(artifact.face.properties.iter().any(|property| {
        property.name == "quantity-unit"
            && property.value == FacePropertyValue::Identity(Unit::Millivolt.semantic_id())
    }));
    assert!(artifact.face.properties.iter().any(|property| {
        property.name == "quantity-value"
            && property.value == FacePropertyValue::Text("-17e0".into())
    }));
    assert!(artifact.face.properties.iter().any(|property| {
        property.name == "quantity-unit-symbol"
            && property.value == FacePropertyValue::Text("mV".into())
    }));
    assert!(artifact.face.text.is_empty());
}

#[test]
fn sequence_face_preserves_exact_minimum_and_maximum_cardinality() {
    let sequence_type =
        StructuredInfoType::bounded_sequence(leaf_type("value/count"), 2, 4).unwrap();
    let value = StructuredInfoValue::sequence(
        sequence_type.clone(),
        vec![
            leaf("value/count", &conduit_core::encode_count(1)),
            leaf("value/count", &conduit_core::encode_count(2)),
        ],
    )
    .unwrap();
    let artifact = StructuredSignFace::from_sign(3, &sign(&value), &sequence_type).unwrap();

    for (name, expected) in [
        ("sequence-item-count", 2),
        ("sequence-minimum-items", 2),
        ("sequence-maximum-items", 4),
    ] {
        assert!(artifact.face.properties.iter().any(|property| {
            property.name == name && property.value == FacePropertyValue::Count(expected)
        }));
    }
    assert!(!artifact
        .face
        .properties
        .iter()
        .any(|property| property.name == "sequence-capacity"));
}

#[test]
fn nominal_face_preserves_semantic_identity_and_representation_shape() {
    let representation_type = leaf_type("value/u8");
    let note_type =
        StructuredInfoType::nominal(KindId::from("music/note@1"), representation_type.clone())
            .unwrap();
    let representation = StructuredInfoValue::leaf(representation_type, vec![60]).unwrap();
    let note = StructuredInfoValue::nominal(note_type.clone(), representation).unwrap();
    let artifact = StructuredSignFace::from_sign(5, &sign(&note), &note_type).unwrap();

    for (name, expected) in [
        ("nominal-schema", "music/note@1"),
        ("leaf-kind", "value/u8"),
    ] {
        assert!(artifact.face.properties.iter().any(|property| {
            property.name == name && property.value == FacePropertyValue::Identity(expected.into())
        }));
    }
}

#[test]
fn declared_data_name_survives_typed_projection_and_invalid_names_are_refused() {
    let quantity = Quantity::new(21, Unit::Celsius);
    let value = leaf(conduit_core::QUANTITY_INFO_ID, &quantity.encode());
    let mut observation = sign(&value);
    let ObservationKind::ValuePresented { label, .. } = &mut observation.kind else {
        unreachable!()
    };
    *label = "Room temperature".into();
    let shown = StructuredSignFace::from_sign(1, &observation, value.value_type()).unwrap();
    assert_eq!(shown.face.subjects[0].name, "Room temperature");
    assert_eq!(
        shown.inspection.value_kind,
        *value.value_type().profile().unwrap().value_kind()
    );
    for invalid in [
        "".to_string(),
        "  ".to_string(),
        "Room\nheat".to_string(),
        "a".repeat(129),
    ] {
        let ObservationKind::ValuePresented { label, .. } = &mut observation.kind else {
            unreachable!()
        };
        *label = invalid;
        assert!(matches!(
            StructuredSignFace::from_sign(1, &observation, value.value_type()),
            Err(conduit_face::StructuredSignFaceRefusal::InvalidDataLabel)
        ));
    }
}
