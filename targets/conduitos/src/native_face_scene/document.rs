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
    pub indent: u8,
}

fn item(text: String, role: GraphicsTextRole) -> Item {
    Item {
        text,
        role,
        paint: GraphicsPaintRole::Foreground,
        control: None,
        indent: 0,
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
        // Contains is Face truth. Its visual indentation is Mask geometry;
        // unknown semantic roles remain named subjects, never dropped.
        let mut depth = 0u8;
        let mut ancestor = subject.identity.as_str();
        while depth < 2 {
            let Some(parent) = face.relationships.iter().find(|relation| {
                relation.target == ancestor
                    && matches!(
                        relation.kind,
                        conduit_presentation::PresentationRelationshipKind::Contains
                    )
            }) else {
                break;
            };
            depth += 1;
            ancestor = &parent.source;
        }
        let mut append = |mut item: Item| {
            item.indent = depth;
            primary.push(item);
        };
        let mut heading = item(subject.name.clone(), role);
        heading.paint = match subject.role {
            PresentationRole::Diagnostic => GraphicsPaintRole::Warning,
            PresentationRole::Status => GraphicsPaintRole::Foreground,
            PresentationRole::Region | PresentationRole::Collection => GraphicsPaintRole::Accent,
            _ => GraphicsPaintRole::Foreground,
        };
        append(heading);
        for text in face
            .text
            .iter()
            .filter(|text| text.subject == subject.identity)
        {
            append(item(text.text.clone(), GraphicsTextRole::Body));
        }
        // The common reader owns the wording and semantic order. A graphical
        // Mask may place these connections beside their source subject, but
        // must not invent a diagram edge or interpret an open domain kind.
        for clause in &plan.clauses {
            let belongs_here = match &clause.provenance {
                Provenance::Relationship(provenance) => {
                    face.relationships[*provenance.index() as usize].source == subject.identity
                }
                Provenance::Composition(provenance) => face.composition.iter().any(|relation| {
                    relation.identity.as_str() == provenance.identity()
                        && relation.source == subject.identity
                }),
                _ => false,
            };
            if belongs_here {
                let mut connection = item(clause.text.clone(), GraphicsTextRole::Body);
                connection.paint = GraphicsPaintRole::Muted;
                append(connection);
            }
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
                    append(item(clause.text.clone(), GraphicsTextRole::Body));
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
                PresentationActionAvailability::Available => (
                    if action.name == subject.name {
                        action.name.clone()
                    } else {
                        format!("{} · {}", action.name, subject.name)
                    },
                    GraphicsPaintRole::Accent,
                ),
                PresentationActionAvailability::Unavailable { explanation, .. }
                | PresentationActionAvailability::Refused { explanation, .. } => (
                    format!("{} — {}", action.name, explanation),
                    GraphicsPaintRole::Warning,
                ),
            };
            append(Item {
                text,
                role: GraphicsTextRole::Action,
                paint,
                control: Some(FaceControl {
                    action: index,
                    argument: None,
                }),
                indent: 0,
            });
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                append(Item {
                    text: format!(
                        "{} · {} · {}",
                        subject.name,
                        argument.value_name,
                        if argument.contract.value_kind.as_str() == "value/bool" {
                            "0 false · 1 true · Enter applies"
                        } else {
                            "enter a replacement value"
                        }
                    ),
                    role: GraphicsTextRole::Label,
                    paint,
                    control: Some(FaceControl {
                        action: index,
                        argument: Some(argument_index),
                    }),
                    indent: 0,
                });
            }
        }
    }
    Ok((primary, details))
}
