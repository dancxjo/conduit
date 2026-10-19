//! Lossless readable projection of the current Birth widget vocabulary.
use super::{BirthFaceRefusal, BirthProjectionBasis};
use crate::{BirthDraft, BirthWidgetFace};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{CheckedValueContract, ValueConstraint, kind_id};
use conduit_face::*;

type Events = Vec<(String, ApplicationEventKind)>;
pub(super) fn project(
    draft: &BirthDraft,
    basis: BirthProjectionBasis<'_>,
) -> Result<(Face, Events), BirthFaceRefusal> {
    if basis.encounter_id().is_empty() || basis.encounter_id().len() > 128 {
        return Err(BirthFaceRefusal::InvalidProjection);
    }
    let root = format!("birth/{}", basis.encounter_id());
    let view = draft.widget_face().map_err(BirthFaceRefusal::View)?;
    let heading = view
        .root
        .children
        .iter()
        .find_map(|node| match &node.mechanism {
            FaceMechanism::Heading { text } => Some(text.clone()),
            _ => None,
        })
        .ok_or(BirthFaceRefusal::UnsupportedMechanism)?;
    let main = format!("{root}/main");
    let article = format!("{root}/article");
    let navigation = format!("{root}/navigation");
    let mut projection = Projection {
        root: root.clone(),
        main: main.clone(),
        article: article.clone(),
        navigation: navigation.clone(),
        subjects: vec![
            FaceSubject {
                identity: root.clone(),
                role: FaceRole::Host,
                name: heading.clone(),
            },
            FaceSubject {
                identity: main.clone(),
                role: FaceRole::Semantic(kind_id("document/main")),
                name: heading,
            },
            FaceSubject {
                identity: article.clone(),
                role: FaceRole::Semantic(kind_id("document/article")),
                name: "Body name and starting Plots".into(),
            },
            FaceSubject {
                identity: navigation.clone(),
                role: FaceRole::Semantic(kind_id("document/navigation")),
                name: "Birth actions".into(),
            },
        ],
        relationships: vec![
            contains(&root, &main),
            contains(&main, &article),
            contains(&main, &navigation),
            // The initial name field is the native Mask's focused control.
            // Put the actions first in every Mask's semantic reading order so
            // reverse navigation reaches Suggest and then Birth.
            FaceRelationship {
                source: navigation.clone(),
                target: article.clone(),
                kind: FaceRelationshipKind::Semantic(kind_id("document/precedes")),
            },
        ],
        text: vec![],
        properties: vec![],
        actions: vec![],
        events: vec![],
    };
    projection.node(&view.root)?;
    let (source_document_id, checked_plot_id, expanded_plot_id, plan_id) = match &basis {
        BirthProjectionBasis::Planned(b) => (
            Some(b.producer_plot.source_document_id.clone()),
            Some(b.producer_plot.checked_plot_id.clone()),
            Some(b.producer_plot.expanded_plot_id.clone()),
            Some(b.producer_plan_id.clone()),
        ),
        BirthProjectionBasis::HostOwned(_) => (None, None, None, None),
    };
    projection.properties.extend([
        FaceProperty {
            subject: root.clone(),
            name: "host-id".into(),
            value: FacePropertyValue::Identity(basis.host_id().as_str().into()),
        },
        FaceProperty {
            subject: root.clone(),
            name: "boot-id".into(),
            value: FacePropertyValue::Identity(basis.boot_id().as_str().into()),
        },
    ]);
    let face = Face::new_with_semantics(
        u64::from(draft.revision()),
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id,
            checked_plot_id,
            expanded_plot_id,
            plan_id,
            active_play_id: None,
            sign_ids: vec![],
        },
        projection.subjects,
        projection.relationships,
        projection.properties,
        projection.text,
        projection.actions,
        vec![
            FaceDisclosure {
                subject: root,
                level: FaceDisclosureLevel::Primary,
            },
            // This named structural container repeats the encounter's heading.
            // Keep its full name and text in every exact reader; the current
            // human-facing title belongs to the Host encounter above.
            FaceDisclosure {
                subject: main,
                level: FaceDisclosureLevel::Context,
            },
        ],
    )
    .map_err(BirthFaceRefusal::Face)?;
    Ok((face, projection.events))
}
struct Projection {
    root: String,
    main: String,
    article: String,
    navigation: String,
    subjects: Vec<FaceSubject>,
    relationships: Vec<FaceRelationship>,
    text: Vec<FaceText>,
    properties: Vec<FaceProperty>,
    actions: Vec<FaceAction>,
    events: Events,
}
fn contains(source: &str, target: &str) -> FaceRelationship {
    FaceRelationship {
        source: source.into(),
        target: target.into(),
        kind: FaceRelationshipKind::Contains,
    }
}
impl Projection {
    fn words(&mut self, subject: &str, text: String) {
        if !text.is_empty() {
            self.text.push(FaceText {
                subject: subject.into(),
                text,
            });
        }
    }
    fn action(
        &mut self,
        subject: &str,
        action: &SemanticAction,
        arguments: Vec<FaceActionArgument>,
    ) {
        self.actions.push(FaceAction {
            identity: action.identity.clone(),
            intent: format!("conduit.intent/{}@1", action.identity),
            target: subject.into(),
            name: action.label.clone(),
            arguments,
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: match &action.availability {
                ActionAvailability::Available => FaceActionAvailability::Available,
                ActionAvailability::Busy { detail } => FaceActionAvailability::Unavailable {
                    reason_code: "birth/busy".into(),
                    explanation: detail.clone(),
                },
                ActionAvailability::Unavailable { detail } => FaceActionAvailability::Unavailable {
                    reason_code: "birth/unavailable".into(),
                    explanation: detail.clone(),
                },
            },
        });
        self.events.push((action.identity.clone(), action.event));
    }
    fn node(&mut self, node: &SemanticFaceNode) -> Result<(), BirthFaceRefusal> {
        let subject = format!("{}/{}", self.root, node.key);
        match &node.mechanism {
            FaceMechanism::Shell => {}
            FaceMechanism::Heading { text } => self.words(&self.main.clone(), text.clone()),
            FaceMechanism::FormField(field) => {
                self.subjects.push(FaceSubject {
                    identity: subject.clone(),
                    role: FaceRole::TextEntry,
                    name: field.label.clone(),
                });
                self.relationships.push(contains(&self.article, &subject));
                self.words(&subject, field.help.clone());
                self.words(
                    &subject,
                    format!(
                        "Current value: {}.",
                        if field.value.is_empty() {
                            "(empty)"
                        } else {
                            &field.value
                        }
                    ),
                );
                if let Some(error) = &field.error {
                    self.words(&subject, format!("Error: {error}"));
                }
                let mut argument = FaceActionArgument::text(
                    "value".into(),
                    field.label.clone(),
                    0,
                    field.value_capacity,
                )
                .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
                match &field.kind {
                    FieldKind::Text => {}
                    FieldKind::NamedSelect { options } => {
                        let mut members: Vec<Vec<u8>> = options
                            .iter()
                            .map(|option| option.identity.as_bytes().to_vec())
                            .collect();
                        members.sort();
                        argument
                            .contract
                            .constraints
                            .push(ValueConstraint::CanonicalMembership {
                                members,
                                negated: false,
                            });
                        for option in options {
                            self.words(
                                &subject,
                                format!(
                                    "Choice: {}. Value: {}. {}.",
                                    option.label,
                                    option.identity,
                                    if option.identity == field.value {
                                        "Selected"
                                    } else {
                                        "Not selected"
                                    }
                                ),
                            );
                        }
                    }
                    _ => return Err(BirthFaceRefusal::UnsupportedMechanism),
                }
                argument
                    .contract
                    .validate(field.value.as_bytes())
                    .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
                // The same Source value supplies readable fallback and exact
                // input state. A Mask need not interpret the fallback prose.
                self.properties.extend([
                    FaceProperty {
                        subject: subject.clone(),
                        name: "current-value".into(),
                        value: FacePropertyValue::TypedValue {
                            contract: argument.contract.clone(),
                            bytes: field.value.as_bytes().to_vec(),
                        },
                    },
                    FaceProperty {
                        subject: subject.clone(),
                        name: "value-change-action".into(),
                        value: FacePropertyValue::Identity(field.input_action.identity.clone()),
                    },
                ]);
                self.action(&subject, &field.input_action, vec![argument]);
            }
            FaceMechanism::ChoiceGroup {
                label,
                options,
                multiplicity,
            } => {
                self.subjects.push(FaceSubject {
                    identity: subject.clone(),
                    role: FaceRole::Collection,
                    name: label.clone(),
                });
                self.relationships.push(contains(&self.article, &subject));
                for (index, option) in options.iter().enumerate() {
                    let id = format!("{subject}/{index}");
                    self.subjects.push(FaceSubject {
                        identity: id.clone(),
                        role: FaceRole::Plot,
                        name: option.label.clone(),
                    });
                    self.relationships.push(contains(&subject, &id));
                    self.properties.push(FaceProperty {
                        subject: id.clone(),
                        name: "selected".into(),
                        value: FacePropertyValue::Flag(option.selected),
                    });
                    // This independent choice really accepts replacement with
                    // either Boolean. It is semantic action association, not
                    // an instruction to any Mask to draw a particular control.
                    if *multiplicity == ChoiceMultiplicity::Independent {
                        self.properties.push(FaceProperty {
                            subject: id.clone(),
                            name: "boolean-change-action".into(),
                            value: FacePropertyValue::Identity(
                                option.change_action.identity.clone(),
                            ),
                        });
                    }
                    self.words(
                        &id,
                        if option.selected {
                            "Included.".into()
                        } else {
                            "Not included.".into()
                        },
                    );
                    let argument = FaceActionArgument {
                        name: "value".into(),
                        value_name: "Include Plot".into(),
                        contract: CheckedValueContract::new(kind_id("value/bool"), 1, vec![])
                            .map_err(|_| BirthFaceRefusal::InvalidProjection)?,
                    };
                    self.action(&id, &option.change_action, vec![argument]);
                }
            }
            FaceMechanism::Action(action) => self.action(&self.navigation.clone(), action, vec![]),
            FaceMechanism::Status { title, detail, .. } => {
                self.subjects.push(FaceSubject {
                    identity: subject.clone(),
                    role: FaceRole::Status,
                    name: title.clone(),
                });
                self.relationships.push(contains(&self.article, &subject));
                self.words(&subject, detail.clone());
            }
            _ => return Err(BirthFaceRefusal::UnsupportedMechanism),
        }
        for child in &node.children {
            self.node(child)?;
        }
        Ok(())
    }
}
