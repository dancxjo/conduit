//! Keep the ordinary view focused on current human wording and usable controls.
//! The exact Face reader remains available in Details, including relationships,
//! properties, unavailable actions, input contracts, and provenance.
use super::*;
use alloc::{format, string::String};
use conduit_presentation::{
    FaceUtteranceProvenance as Provenance, PresentationActionAvailability,
    PresentationDisclosureLevel, PresentationRole, plan_face_utterances,
    readable_finite_text_choices,
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

pub(super) fn prepare(
    face: &Presentation,
    interaction_admitted: bool,
) -> Result<(Vec<Item>, Vec<Item>), FaceSceneError> {
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
            detail.control = interaction_admitted.then_some(FaceControl {
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
            detail.paint = if interaction_admitted && action.availability.is_available() {
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
                && (matches!(
                    disclosure.level,
                    PresentationDisclosureLevel::CurrentAction
                        | PresentationDisclosureLevel::SelectedDetail
                        | PresentationDisclosureLevel::ExactProvenance
                ) || (disclosure.level == PresentationDisclosureLevel::Context
                    && !matches!(subject.role, PresentationRole::Collection | PresentationRole::Document)))
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
        // The Body name anchors the encounter. Its generic lifecycle and
        // resident-count sentence is inspection context; application subjects
        // supply the ordinary view's useful content.
        if subject.role != PresentationRole::Body {
            for text in face
                .text
                .iter()
                .filter(|text| text.subject == subject.identity)
            {
                append(item(text.text.clone(), GraphicsTextRole::Body));
            }
        }
        // Contains already determines indentation. Readable primary wording
        // comes from this subject's Face text; technical relationships and
        // properties remain inspectable in Details without becoming a second
        // copy of the main view.
        // Action order comes from the Face, never an application-name convention.
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
            if !matches!(
                action.availability,
                PresentationActionAvailability::Available
            ) {
                continue;
            }
            let paint = if interaction_admitted {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
            append(Item {
                text: if interaction_admitted {
                    action.name.clone()
                } else {
                    format!("{} · View only on this host", action.name)
                },
                role: GraphicsTextRole::Action,
                paint,
                control: interaction_admitted.then_some(FaceControl {
                    action: index,
                    argument: None,
                }),
                indent: 0,
            });
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                let hint = if argument.contract.value_kind.as_str() == "value/bool" {
                    "false or true".into()
                } else {
                    argument_hint(argument)
                };
                append(Item {
                    text: format!("{} · {}", argument.value_name, hint),
                    role: GraphicsTextRole::Label,
                    paint,
                    control: interaction_admitted.then_some(FaceControl {
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

fn argument_hint(argument: &conduit_presentation::FaceActionArgument) -> String {
    readable_finite_text_choices(&argument.contract).map_or_else(
        || "Enter a value".into(),
        |choices| format!("Choose {}", choices.join(", ")),
    )
}
