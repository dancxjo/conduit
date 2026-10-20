//! Lossless readable projection of the current Birth widget vocabulary.
use super::{BirthFaceRefusal, BirthProjectionBasis};
use crate::{BirthDraft, BirthWidgetFace};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{CheckedValueContract, kind_id};
use conduit_face::*;
use sha2::{Digest, Sha256};

mod fields;

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
        disclosures: vec![
            FaceDisclosure {
                subject: root.clone(),
                level: FaceDisclosureLevel::Primary,
            },
            // Preserve the structural name without repeating the primary title.
            FaceDisclosure {
                subject: main.clone(),
                level: FaceDisclosureLevel::Context,
            },
        ],
        selected_review_in_primary: !draft.search().trim().is_empty(),
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
        projection.disclosures,
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
    disclosures: Vec<FaceDisclosure>,
    selected_review_in_primary: bool,
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
            // The Birth owner selects its primary commitment. Masks need no
            // application action-name convention to distinguish it visually.
            disclosure: if action.identity == "creche.birth" {
                FaceDisclosureLevel::Primary
            } else {
                FaceDisclosureLevel::CurrentAction
            },
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
            FaceMechanism::FormField(field) => self.field(subject, field)?,
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
                for option in options {
                    // Checked Plot identity survives filtering and catalogue reorder.
                    let digest = Sha256::digest(option.identity.as_bytes());
                    let id = format!("{subject}/{digest:x}");
                    self.subjects.push(FaceSubject {
                        identity: id.clone(),
                        role: FaceRole::Plot,
                        name: option.label.clone(),
                    });
                    self.relationships.push(contains(&subject, &id));
                    // Independent selection is already primary typed meaning.
                    // Keep its readable fallback explicitly inspectable without
                    // requiring any Mask to infer state from these words.
                    let selection_details = format!("{id}/selection-details");
                    self.subjects.push(FaceSubject {
                        identity: selection_details.clone(),
                        role: FaceRole::Info,
                        name: format!("{} selection details", option.label),
                    });
                    self.relationships.push(contains(&id, &selection_details));
                    self.disclosures.push(FaceDisclosure {
                        subject: selection_details.clone(),
                        level: FaceDisclosureLevel::SelectedDetail,
                    });
                    self.properties.push(FaceProperty {
                        subject: id.clone(),
                        name: "checked-plot-id".into(),
                        value: FacePropertyValue::Identity(option.identity.clone()),
                    });
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
                        &selection_details,
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
                // The unfiltered checked inventory already names every selected
                // Plot. A filter must never make that review disappear.
                if node.key.starts_with("birth-review-plot-") && !self.selected_review_in_primary {
                    self.disclosures.push(FaceDisclosure {
                        subject: subject.clone(),
                        level: FaceDisclosureLevel::SelectedDetail,
                    });
                }
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
