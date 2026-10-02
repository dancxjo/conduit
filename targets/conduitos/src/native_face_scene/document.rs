//! Reuse the generic Face reader's order and exact provenance. Primary graphics
//! group human wording by subject; full contract/identity clauses remain in Details.
use super::*;
use alloc::{format, string::String};
use conduit_presentation::{
    FaceUtteranceProvenance as Provenance, PresentationActionAvailability,
    PresentationDisclosureLevel, PresentationPropertyValue, PresentationRole, plan_face_utterances,
};

pub(super) struct Item {
    pub text: String,
    pub role: GraphicsTextRole,
    pub paint: GraphicsPaintRole,
    pub control: Option<FaceControl>,
}

fn item(text: String, role: GraphicsTextRole) -> Item {
    Item {
        text,
        role,
        paint: GraphicsPaintRole::Foreground,
        control: None,
    }
}

pub(super) fn prepare(face: &Presentation) -> Result<(Vec<Item>, Vec<Item>), FaceSceneError> {
    let plan = plan_face_utterances(face).map_err(|_| FaceSceneError::DocumentBound)?;
    let mut primary = Vec::new();
    let mut details = Vec::with_capacity(plan.clauses.len());
    for clause in &plan.clauses {
        let mut detail = item(clause.text.clone(), GraphicsTextRole::Body);
        let action_name = match &clause.provenance {
            Provenance::Action(value) => Some((value.identity(), None)),
            Provenance::ActionArgument(value) => {
                Some((value.action_identity(), Some(value.argument_name())))
            }
            _ => None,
        };
        if let Some((name, argument_name)) = action_name {
            let (index, action) = face
                .actions
                .iter()
                .enumerate()
                .find(|(_, action)| &action.identity == name)
                .ok_or(FaceSceneError::InvalidFace)?;
            detail.control = Some(FaceControl {
                action: index,
                argument: argument_name
                    .map(|name| {
                        action
                            .arguments
                            .iter()
                            .position(|argument| &argument.name == name)
                            .ok_or(FaceSceneError::InvalidFace)
                    })
                    .transpose()?,
            });
            detail.paint = if action.availability.is_available() {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
        }
        details.push(detail);
        let Provenance::Subject(provenance) = &clause.provenance else {
            continue;
        };
        let subject = face
            .subjects
            .iter()
            .find(|subject| subject.identity.as_str() == provenance.identity())
            .ok_or(FaceSceneError::InvalidFace)?;
        if face.disclosures.iter().any(|disclosure| {
            disclosure.subject == subject.identity
                && matches!(
                    disclosure.level,
                    PresentationDisclosureLevel::SelectedDetail
                        | PresentationDisclosureLevel::ExactProvenance
                )
        }) {
            continue;
        }
        let role = match subject.role {
            PresentationRole::Document | PresentationRole::Body | PresentationRole::Host => {
                GraphicsTextRole::Title
            }
            PresentationRole::Region | PresentationRole::Collection => GraphicsTextRole::Heading,
            PresentationRole::Status => GraphicsTextRole::Status,
            PresentationRole::Diagnostic => GraphicsTextRole::Warning,
            _ => GraphicsTextRole::Label,
        };
        primary.push(item(subject.name.clone(), role));
        for text in face
            .text
            .iter()
            .filter(|text| text.subject == subject.identity)
        {
            primary.push(item(text.text.clone(), GraphicsTextRole::Body));
        }
        for clause in &plan.clauses {
            if let Provenance::Property(provenance) = &clause.provenance {
                let property = &face.properties[*provenance.index() as usize];
                if property.subject == subject.identity
                    && matches!(
                        property.value,
                        PresentationPropertyValue::Text(_)
                            | PresentationPropertyValue::Count(_)
                            | PresentationPropertyValue::Signed(_)
                            | PresentationPropertyValue::Flag(_)
                    )
                {
                    primary.push(item(clause.text.clone(), GraphicsTextRole::Body));
                }
            }
        }
        // Action order comes from the same semantic subject ordering as the
        // common reader, never an application's action-name convention.
        for clause in &plan.clauses {
            let Provenance::Action(provenance) = &clause.provenance else {
                continue;
            };
            let (index, action) = face
                .actions
                .iter()
                .enumerate()
                .find(|(_, action)| action.identity.as_str() == provenance.identity())
                .ok_or(FaceSceneError::InvalidFace)?;
            if action.target != subject.identity {
                continue;
            }
            let (text, paint) = match &action.availability {
                PresentationActionAvailability::Available => {
                    (action.name.clone(), GraphicsPaintRole::Accent)
                }
                PresentationActionAvailability::Unavailable { explanation, .. }
                | PresentationActionAvailability::Refused { explanation, .. } => (
                    format!("{} — {}", action.name, explanation),
                    GraphicsPaintRole::Muted,
                ),
            };
            primary.push(Item {
                text,
                role: GraphicsTextRole::Action,
                paint,
                control: Some(FaceControl {
                    action: index,
                    argument: None,
                }),
            });
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                primary.push(Item {
                    text: format!(
                        "{} · {}",
                        argument.value_name,
                        if argument.contract.value_kind.as_str() == "value/bool" {
                            "0 false · 1 true · Enter applies"
                        } else {
                            "enter a replacement value"
                        }
                    ),
                    role: GraphicsTextRole::Body,
                    paint,
                    control: Some(FaceControl {
                        action: index,
                        argument: Some(argument_index),
                    }),
                });
            }
        }
    }
    Ok((primary, details))
}
