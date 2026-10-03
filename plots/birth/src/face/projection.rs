//! Lossless readable projection of the current Birth widget vocabulary.
use super::{BirthFaceRefusal, FaceBasis};
use crate::{BirthDraft, BirthPresentation};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{CheckedValueContract, ValueConstraint, kind_id};
use conduit_presentation::*;

type Events = Vec<(String, ApplicationEventKind)>;
pub(super) fn project(
    draft: &BirthDraft,
    basis: FaceBasis<'_>,
) -> Result<(Presentation, Events), BirthFaceRefusal> {
    if basis.encounter_id().is_empty() || basis.encounter_id().len() > 128 {
        return Err(BirthFaceRefusal::InvalidProjection);
    }
    let root = format!("birth/{}", basis.encounter_id());
    let view = draft.presentation().map_err(BirthFaceRefusal::View)?;
    let heading = view
        .root
        .children
        .iter()
        .find_map(|node| match &node.mechanism {
            PresentationMechanism::Heading { text } => Some(text.clone()),
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
            PresentationSubject {
                identity: root.clone(),
                role: PresentationRole::Host,
                name: heading.clone(),
            },
            PresentationSubject {
                identity: main.clone(),
                role: PresentationRole::Semantic(kind_id("document/main")),
                name: heading,
            },
            PresentationSubject {
                identity: article.clone(),
                role: PresentationRole::Semantic(kind_id("document/article")),
                name: "Body name and starting Plots".into(),
            },
            PresentationSubject {
                identity: navigation.clone(),
                role: PresentationRole::Semantic(kind_id("document/navigation")),
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
            PresentationRelationship {
                source: navigation.clone(),
                target: article.clone(),
                kind: PresentationRelationshipKind::Semantic(kind_id("document/precedes")),
            },
        ],
        text: vec![],
        actions: vec![],
        events: vec![],
    };
    projection.node(&view.root)?;
    let (source_document_id, checked_plot_id, expanded_plot_id, plan_id) = match &basis {
        FaceBasis::Planned(b) => (
            Some(b.producer_plot.source_document_id.clone()),
            Some(b.producer_plot.checked_plot_id.clone()),
            Some(b.producer_plot.expanded_plot_id.clone()),
            Some(b.producer_plan_id.clone()),
        ),
        FaceBasis::HostOwned(_) => (None, None, None, None),
    };
    let face = Presentation::new_with_semantics(
        u64::from(draft.revision()),
        PresentationBasis {
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
        vec![
            PresentationProperty {
                subject: root.clone(),
                name: "host-id".into(),
                value: PresentationPropertyValue::Identity(basis.host_id().as_str().into()),
            },
            PresentationProperty {
                subject: root.clone(),
                name: "boot-id".into(),
                value: PresentationPropertyValue::Identity(basis.boot_id().as_str().into()),
            },
        ],
        projection.text,
        projection.actions,
        vec![PresentationDisclosure {
            subject: root,
            level: PresentationDisclosureLevel::Primary,
        }],
    )
    .map_err(BirthFaceRefusal::Presentation)?;
    Ok((face, projection.events))
}
struct Projection {
    root: String,
    main: String,
    article: String,
    navigation: String,
    subjects: Vec<PresentationSubject>,
    relationships: Vec<PresentationRelationship>,
    text: Vec<PresentationText>,
    actions: Vec<PresentationAction>,
    events: Events,
}
fn contains(source: &str, target: &str) -> PresentationRelationship {
    PresentationRelationship {
        source: source.into(),
        target: target.into(),
        kind: PresentationRelationshipKind::Contains,
    }
}
impl Projection {
    fn words(&mut self, subject: &str, text: String) {
        if !text.is_empty() {
            self.text.push(PresentationText {
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
        self.actions.push(PresentationAction {
            identity: action.identity.clone(),
            intent: format!("conduit.intent/{}@1", action.identity),
            target: subject.into(),
            name: action.label.clone(),
            arguments,
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: match &action.availability {
                ActionAvailability::Available => PresentationActionAvailability::Available,
                ActionAvailability::Busy { detail } => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "birth/busy".into(),
                        explanation: detail.clone(),
                    }
                }
                ActionAvailability::Unavailable { detail } => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "birth/unavailable".into(),
                        explanation: detail.clone(),
                    }
                }
            },
        });
        self.events.push((action.identity.clone(), action.event));
    }
    fn node(&mut self, node: &SemanticPresentationNode) -> Result<(), BirthFaceRefusal> {
        let subject = format!("{}/{}", self.root, node.key);
        match &node.mechanism {
            PresentationMechanism::Shell => {}
            PresentationMechanism::Heading { text } => self.words(&self.main.clone(), text.clone()),
            PresentationMechanism::FormField(field) => {
                self.subjects.push(PresentationSubject {
                    identity: subject.clone(),
                    role: PresentationRole::TextEntry,
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
                self.action(&subject, &field.input_action, vec![argument]);
            }
            PresentationMechanism::ChoiceGroup { label, options, .. } => {
                self.subjects.push(PresentationSubject {
                    identity: subject.clone(),
                    role: PresentationRole::Collection,
                    name: label.clone(),
                });
                self.relationships.push(contains(&self.article, &subject));
                for (index, option) in options.iter().enumerate() {
                    let id = format!("{subject}/{index}");
                    self.subjects.push(PresentationSubject {
                        identity: id.clone(),
                        role: PresentationRole::Plot,
                        name: option.label.clone(),
                    });
                    self.relationships.push(contains(&subject, &id));
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
            PresentationMechanism::Action(action) => {
                self.action(&self.navigation.clone(), action, vec![])
            }
            PresentationMechanism::Status { title, detail, .. } => {
                self.subjects.push(PresentationSubject {
                    identity: subject.clone(),
                    role: PresentationRole::Status,
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
