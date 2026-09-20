use conduit_core::{
    encode_count, BootId, HostId, KindId, Observation, ObservationKind, PrimitiveInfoRefusal,
    Quantity, QuantityDecodeRefusal, QuantityUnit, SignId, StructuredFieldType,
    StructuredFieldValue, StructuredInfoInspection, StructuredInfoInspectionRefusal,
    StructuredInfoInspectionShape, StructuredInfoLeafSemantic, StructuredInfoType,
    StructuredInfoValue, ValuePayload, MAXIMUM_STRUCTURED_INSPECTION_NODES,
};

fn leaf_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from(kind)).unwrap()
}

fn leaf(kind: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf_type(kind), bytes.to_vec()).unwrap()
}

fn sign(value: &StructuredInfoValue) -> Observation {
    Observation {
        sign_id: SignId::from("sign/structured"),
        active_play_id: None,
        presentation_id: None,
        host_id: HostId::from("host/local"),
        boot_id: BootId::from("boot/1"),
        plan_id: None,
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::ValueProduced {
            value: ValuePayload {
                value_kind: value.value_type().profile().unwrap().value_kind().clone(),
                encoded: value.canonical_bytes().unwrap(),
            },
        },
    }
}

#[test]
fn music_and_llm_signs_share_one_leaf_redacting_inspection() {
    let note_type = StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![
            StructuredFieldType::new("pitch", leaf_type("value/count")).unwrap(),
            StructuredFieldType::new("velocity", leaf_type("value/count")).unwrap(),
        ],
    )
    .unwrap();
    let note = StructuredInfoValue::record(
        note_type.clone(),
        vec![
            StructuredFieldValue::new("pitch", leaf("value/count", &encode_count(60))).unwrap(),
            StructuredFieldValue::new("velocity", leaf("value/count", &encode_count(101))).unwrap(),
        ],
    )
    .unwrap();
    let extraction_type = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![StructuredFieldType::new("private_answer", leaf_type("value/text")).unwrap()],
    )
    .unwrap();
    let extraction = StructuredInfoValue::record(
        extraction_type.clone(),
        vec![
            StructuredFieldValue::new("private_answer", leaf("value/text", b"do-not-retain"))
                .unwrap(),
        ],
    )
    .unwrap();

    for value in [&note, &extraction] {
        let inspection = StructuredInfoInspection::from_sign(&sign(value), value.value_type())
            .expect("ordinary value Sign is inspectable");
        assert_eq!(inspection.omitted_nodes, 0);
        assert_eq!(inspection.value_digest, value.semantic_digest().unwrap());
        assert!(inspection.nodes.iter().any(|node| matches!(
            node.shape,
            StructuredInfoInspectionShape::Leaf { byte_len, .. } if byte_len > 0
        )));
        let rendered = format!("{inspection:?}");
        assert!(!rendered.contains("do-not-retain"));
    }
}

#[test]
fn inspection_has_a_tighter_cap_and_reports_every_omitted_node() {
    let collection_type = StructuredInfoType::collection(leaf_type("value/count"), Some(8))
        .expect("finite collection type");
    let fields: Vec<_> = (0..64)
        .map(|index| {
            StructuredFieldType::new(format!("field-{index:02}"), collection_type.clone()).unwrap()
        })
        .collect();
    let record_type =
        StructuredInfoType::record(KindId::from("test/large-record@1"), fields).unwrap();
    let values: Vec<_> = (0..64)
        .map(|index| {
            let items = (0..8)
                .map(|item| leaf("value/count", &encode_count(index * 8 + item)))
                .collect();
            StructuredFieldValue::new(
                format!("field-{index:02}"),
                StructuredInfoValue::collection(collection_type.clone(), items).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let record = StructuredInfoValue::record(record_type.clone(), values).unwrap();

    let inspection = StructuredInfoInspection::from_sign(&sign(&record), &record_type).unwrap();
    assert_eq!(inspection.nodes.len(), MAXIMUM_STRUCTURED_INSPECTION_NODES);
    assert_eq!(inspection.omitted_nodes, 65);
}

#[test]
fn non_value_malformed_and_wrong_profile_signs_refuse_distinctly() {
    let expected = leaf_type("value/text");
    let mut non_value = sign(&leaf("value/text", b"ok"));
    non_value.kind = ObservationKind::PlanCompleted;
    assert_eq!(
        StructuredInfoInspection::from_sign(&non_value, &expected),
        Err(StructuredInfoInspectionRefusal::NotValueSign)
    );

    let mut wrong_profile = sign(&leaf("value/text", b"ok"));
    let ObservationKind::ValueProduced { value } = &mut wrong_profile.kind else {
        unreachable!()
    };
    value.value_kind = KindId::from("structured-info/profile-wrong@1");
    assert_eq!(
        StructuredInfoInspection::from_sign(&wrong_profile, &expected),
        Err(StructuredInfoInspectionRefusal::ProfileMismatch)
    );

    let mut malformed = sign(&leaf("value/text", b"ok"));
    let ObservationKind::ValueProduced { value } = &mut malformed.kind else {
        unreachable!()
    };
    value.encoded.truncate(3);
    assert!(matches!(
        StructuredInfoInspection::from_sign(&malformed, &expected),
        Err(StructuredInfoInspectionRefusal::InvalidStructuredValue(_))
    ));
}

#[test]
fn quantity_signs_retain_exact_typed_semantics_without_general_leaf_disclosure() {
    let quantity = Quantity::new(-17, QuantityUnit::Millivolt);
    let value = leaf(conduit_core::QUANTITY_INFO_ID, &quantity.encode());
    let inspection =
        StructuredInfoInspection::from_sign(&sign(&value), value.value_type()).unwrap();
    assert!(matches!(
        &inspection.nodes[0].shape,
        StructuredInfoInspectionShape::Leaf {
            semantic: Some(StructuredInfoLeafSemantic::Quantity(observed)),
            ..
        } if *observed == quantity
    ));

    let quantity_type = leaf_type(conduit_core::QUANTITY_INFO_ID);
    assert_eq!(
        StructuredInfoValue::leaf(quantity_type, vec![0; 8]),
        Err(conduit_core::StructuredInfoRefusal::InvalidPrimitiveLeaf(
            PrimitiveInfoRefusal::Quantity(QuantityDecodeRefusal::WrongLength {
                expected: conduit_core::QUANTITY_ENCODED_LEN,
                actual: 8,
            })
        ))
    );
}
