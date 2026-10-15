//! Documentary Face fixtures; real peer/kernel acknowledgement is proved separately.
use super::*;

fn control_face() -> Face {
    let subject = |id: &str, role, name: &str| FaceSubject {
        identity: id.into(),
        role,
        name: name.into(),
    };
    let quantity =
        conduit_core::Quantity::from_decimal(225, -1, conduit_core::Unit::Celsius).unwrap();
    let contract = conduit_core::CheckedValueContract::new(
        conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
        conduit_core::QUANTITY_ENCODED_LEN as u32,
        vec![],
    )
    .unwrap();
    Face::new_with_semantics(
        1,
        face(1).basis,
        vec![
            subject("owner", FaceRole::Body, "Owner context"),
            subject("region", FaceRole::Region, "Lab controller"),
            subject("target", FaceRole::Info, "Requested setpoint"),
            subject("sensor", FaceRole::Status, "Sensor"),
        ],
        vec![
            FaceRelationship {
                source: "owner".into(),
                target: "region".into(),
                kind: FaceRelationshipKind::Contains,
            },
            FaceRelationship {
                source: "region".into(),
                target: "target".into(),
                kind: FaceRelationshipKind::Contains,
            },
            FaceRelationship {
                source: "region".into(),
                target: "sensor".into(),
                kind: FaceRelationshipKind::Contains,
            },
        ],
        vec![FaceProperty {
            subject: "target".into(),
            name: "value".into(),
            value: FacePropertyValue::TypedValue {
                contract,
                bytes: quantity.encode().to_vec(),
            },
        }],
        vec![
            FaceText {
                subject: "owner".into(),
                text: "Administrative context".into(),
            },
            FaceText {
                subject: "sensor".into(),
                text: "Sensor unavailable".into(),
            },
        ],
        vec![
            FaceAction {
                identity: "lull".into(),
                intent: "fixture/lull".into(),
                target: "owner".into(),
                name: "Lull Body".into(),
                arguments: vec![],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            },
            FaceAction {
                identity: "raise".into(),
                intent: "fixture/raise".into(),
                target: "target".into(),
                name: "Raise requested setting".into(),
                arguments: vec![],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            },
        ],
        ["owner", "region", "target", "sensor"]
            .into_iter()
            .map(|id| FaceDisclosure {
                subject: id.into(),
                level: if id == "owner" {
                    FaceDisclosureLevel::Context
                } else {
                    FaceDisclosureLevel::Primary
                },
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn primary_region_shows_typed_values_and_keeps_body_management_in_inspect() {
    let original = control_face();
    let mask = TerminalFaceMask::prepare(original.clone(), 80, 24).unwrap();
    let document = mask
        .document
        .iter()
        .map(|row| row.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(document.contains("Requested setpoint: 22.5 °C"));
    assert!(mask
        .document
        .iter()
        .find(|row| row.text.contains("22.5 °C"))
        .unwrap()
        .clause
        .is_some());
    assert!(document.contains("Sensor unavailable"));
    assert!(document.contains("Raise requested setting"));
    assert!(!document.contains("Lull Body"));
    assert!(!document.contains("Administrative context"));
    assert!(mask
        .inspect_document
        .iter()
        .any(|row| row.text.contains("Lull Body")));
    assert_eq!(mask.face(), &original);
    assert!(document::frame(&mask).contains("Lab controller"));
}

#[test]
fn selected_detail_values_remain_inspectable_without_entering_the_primary_view() {
    let mut face = control_face();
    face.disclosures
        .iter_mut()
        .find(|item| item.subject == "target")
        .unwrap()
        .level = FaceDisclosureLevel::SelectedDetail;
    // Recompute the exact Face identity after this fixture's disclosure change.
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare(face, 80, 24).unwrap();
    assert!(!mask.document.iter().any(|row| row.text.contains("22.5 °C")));
    assert!(mask
        .inspect_document
        .iter()
        .any(|row| row.text.contains("22.5 °C")));
}

#[test]
fn multiple_primary_regions_preserve_all_content_instead_of_selecting_one() {
    let mut face = control_face();
    face.subjects.push(FaceSubject {
        identity: "other".into(),
        role: FaceRole::Region,
        name: "Other region".into(),
    });
    face.relationships.push(FaceRelationship {
        source: "owner".into(),
        target: "other".into(),
        kind: FaceRelationshipKind::Contains,
    });
    face.text.push(FaceText {
        subject: "other".into(),
        text: "Other declared content".into(),
    });
    face.disclosures.push(FaceDisclosure {
        subject: "other".into(),
        level: FaceDisclosureLevel::Primary,
    });
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare(face, 80, 24).unwrap();
    assert!(mask
        .document
        .iter()
        .any(|row| row.text.contains("Other declared content")));
    assert!(mask.document.iter().any(|row| row.text.contains("22.5 °C")));
}

#[test]
fn semantic_choice_actions_include_their_exact_group_name() {
    let mut face = control_face();
    face.subjects
        .iter_mut()
        .find(|subject| subject.identity == "target")
        .unwrap()
        .role = FaceRole::Semantic(conduit_core::kind_id(
        FaceMechanismKind::ChoiceGroup.identity(),
    ));
    face.actions
        .iter_mut()
        .find(|action| action.identity == "raise")
        .unwrap()
        .name = "Auto".into();
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare(face, 80, 24).unwrap();
    assert!(mask
        .document
        .iter()
        .any(|row| row.text == "Auto · Requested setpoint"));
}
