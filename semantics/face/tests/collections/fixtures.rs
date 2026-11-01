use conduit_core::{kind_id, CheckedValueContract, ValueConstraint};
use conduit_face::*;

pub fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}
pub fn change(face: &mut Face, subject: &str, name: &str, value: FacePropertyValue) {
    face.properties
        .iter_mut()
        .find(|p| p.subject == subject && p.name == name)
        .unwrap()
        .value = value;
}
pub fn rebuild(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}
/// Canonical semantic fixture only; it is not a domain owner or platform proof.
pub fn specimen(mode: Option<ChoiceMultiplicity>, count: usize) -> Face {
    let mut subjects = vec![FaceSubject {
        identity: "list".into(),
        role: FaceRole::Collection,
        name: "Source choices".into(),
    }];
    let mut properties = vec![property(
        "list",
        "item-count",
        FacePropertyValue::Count(count as u64),
    )];
    let mut relationships = vec![];
    let mut actions = vec![];
    let values: Vec<Vec<u8>> = (0..count)
        .map(|index| format!("value/{index}").into_bytes())
        .collect();
    if let Some(mode) = mode {
        properties.push(property(
            "list",
            "choice-multiplicity",
            FacePropertyValue::Text(
                match mode {
                    ChoiceMultiplicity::Exclusive => "exclusive",
                    ChoiceMultiplicity::Independent => "independent",
                }
                .into(),
            ),
        ));
    }
    if mode == Some(ChoiceMultiplicity::Exclusive) {
        subjects.push(FaceSubject {
            identity: "source".into(),
            role: FaceRole::TextEntry,
            name: "Selected value".into(),
        });
        let contract = CheckedValueContract::new(
            kind_id("value/text"),
            16,
            vec![ValueConstraint::CanonicalMembership {
                members: values.clone(),
                negated: false,
            }],
        )
        .unwrap();
        properties.extend([
            property(
                "list",
                "minimum-selected-count",
                FacePropertyValue::Count(1),
            ),
            property(
                "list",
                "maximum-selected-count",
                FacePropertyValue::Count(1),
            ),
            property(
                "list",
                "selection-action",
                FacePropertyValue::Identity("source/replace".into()),
            ),
            property(
                "list",
                "value-source",
                FacePropertyValue::Identity("source".into()),
            ),
            property(
                "source",
                "value-change-action",
                FacePropertyValue::Identity("source/replace".into()),
            ),
            property(
                "source",
                "current-value",
                FacePropertyValue::TypedValue {
                    contract: contract.clone(),
                    bytes: values[0].clone(),
                },
            ),
        ]);
        actions.push(FaceAction {
            identity: "source/replace".into(),
            intent: "test/replace".into(),
            target: "source".into(),
            name: "Choose".into(),
            arguments: vec![FaceActionArgument {
                name: "exact/source-argument".into(),
                value_name: "Choice".into(),
                contract,
            }],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        });
    }
    for (index, value) in values.into_iter().enumerate() {
        let id = format!("opaque/{}", count - index);
        subjects.push(FaceSubject {
            identity: id.clone(),
            role: FaceRole::Item,
            name: "同じ · étiquette".into(),
        });
        relationships.push(FaceRelationship {
            source: "list".into(),
            target: id.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        properties.push(property(
            &id,
            "order",
            FacePropertyValue::Count((index * 3) as u64),
        ));
        if mode.is_some() {
            properties.push(property(
                &id,
                "selected",
                FacePropertyValue::Flag(index == 0),
            ));
        }
        match mode {
            Some(ChoiceMultiplicity::Exclusive) => properties.push(property(
                &id,
                "choice-value",
                FacePropertyValue::TypedValue {
                    contract: CheckedValueContract::new(kind_id("value/text"), 16, vec![]).unwrap(),
                    bytes: value,
                },
            )),
            Some(ChoiceMultiplicity::Independent) => {
                let action = format!("source/replace/{index}");
                properties.push(property(
                    &id,
                    "boolean-change-action",
                    FacePropertyValue::Identity(action.clone()),
                ));
                actions.push(FaceAction {
                    identity: action,
                    intent: "test/replace".into(),
                    target: id,
                    name: "Set selection".into(),
                    arguments: vec![FaceActionArgument {
                        name: "exact/source-boolean".into(),
                        value_name: "Selected".into(),
                        contract: CheckedValueContract::new(kind_id("value/bool"), 1, vec![])
                            .unwrap(),
                    }],
                    disclosure: FaceDisclosureLevel::CurrentAction,
                    availability: FaceActionAvailability::Available,
                });
            }
            None => {}
        }
    }
    subjects.reverse();
    relationships.reverse();
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        subjects,
        relationships,
        properties,
        vec![],
        actions,
        vec![],
    )
    .unwrap()
}
