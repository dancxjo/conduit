//! Keep the ordinary view focused on current human wording and usable controls.
//! The exact Face reader remains available in Details, including relationships,
//! properties, unavailable actions, input contracts, and provenance.
use super::*;
use alloc::{format, string::String};
use conduit_face::{
    FaceActionAvailability, FaceDisclosureLevel, FacePropertyValue, FaceRole,
    FaceUtteranceProvenance as Provenance, plan_face_utterances, readable_finite_text_choices,
};

pub(super) struct Item {
    pub text: String,
    pub role: GraphicsTextRole,
    pub paint: GraphicsPaintRole,
    pub control: Option<FaceControl>,
    pub indent: u8,
    pub subject_index: Option<usize>,
    pub region: Option<u8>,
    pub checkbox: Option<bool>,
}

fn item(text: String, role: GraphicsTextRole) -> Item {
    Item {
        text,
        role,
        paint: GraphicsPaintRole::Foreground,
        control: None,
        indent: 0,
        subject_index: None,
        region: None,
        checkbox: None,
    }
}

pub(super) fn prepare(
    face: &Face,
    interaction_admitted: bool,
) -> Result<(Vec<Item>, Vec<Item>), FaceSceneError> {
    let plan = plan_face_utterances(face).map_err(|_| FaceSceneError::DocumentBound)?;
    let mut primary = Vec::new();
    let mut details = Vec::with_capacity(plan.clauses.len());
    for (index, subject) in face.subjects.iter().enumerate().filter(|(_, subject)| {
        matches!(
            subject.role,
            FaceRole::Gear | FaceRole::Port | FaceRole::Cord
        )
    }) {
        let mut identity = item(
            format!("{:?} identity: {}", subject.role, subject.identity),
            GraphicsTextRole::Body,
        );
        identity.subject_index = Some(index);
        details.push(identity);
        let mut name = item(format!("Name: {}", subject.name), GraphicsTextRole::Body);
        name.subject_index = Some(index);
        details.push(name);
        if subject.role == FaceRole::Cord {
            let mut endpoints = item(
                super::ports::cord_summary(face, subject),
                GraphicsTextRole::Body,
            );
            endpoints.subject_index = Some(index);
            details.push(endpoints);
            for relation in face.relationships.iter().filter(|relation| {
                relation.source == subject.identity
                    && relation.kind == conduit_face::FaceRelationshipKind::Connects
            }) {
                let mut endpoint = item(
                    format!("{} connects {}", subject.identity, relation.target),
                    GraphicsTextRole::Body,
                );
                endpoint.subject_index = Some(index);
                details.push(endpoint);
            }
        }
    }
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
                    FaceDisclosureLevel::CurrentAction
                        | FaceDisclosureLevel::SelectedDetail
                        | FaceDisclosureLevel::ExactProvenance
                ) || (disclosure.level == FaceDisclosureLevel::Context
                    && !matches!(subject.role, FaceRole::Collection | FaceRole::Document)))
        }) {
            continue;
        }
        let role = match subject.role {
            FaceRole::Document | FaceRole::Body | FaceRole::Host => GraphicsTextRole::Title,
            FaceRole::Region | FaceRole::Collection => GraphicsTextRole::Heading,
            FaceRole::Status => GraphicsTextRole::Status,
            FaceRole::Diagnostic => GraphicsTextRole::Warning,
            _ => GraphicsTextRole::Label,
        };
        // Contains is Face truth. Its visual indentation is Mask geometry;
        // unknown semantic roles remain named subjects, never dropped.
        let mut depth = 0u8;
        let mut ancestor = subject.identity.as_str();
        while depth < 2 {
            let Some(parent) = face.relationships.iter().find(|relation| {
                relation.target == ancestor
                    && matches!(relation.kind, conduit_face::FaceRelationshipKind::Contains)
            }) else {
                break;
            };
            depth += 1;
            ancestor = &parent.source;
        }
        let mut append = |mut item: Item| {
            item.indent = depth;
            item.subject_index = face
                .subjects
                .iter()
                .position(|candidate| candidate.identity == subject.identity);
            primary.push(item);
        };
        let name = if subject.role == FaceRole::Item {
            face.properties
                .iter()
                .find(|property| {
                    property.subject == subject.identity && property.name == "complete"
                })
                .and_then(|property| match &property.value {
                    FacePropertyValue::Flag(complete) => Some(*complete),
                    _ => None,
                })
                .map(|complete| format!("[{}] {}", if complete { 'x' } else { ' ' }, subject.name))
                .unwrap_or_else(|| subject.name.clone())
        } else {
            subject.name.clone()
        };
        let mut heading = item(name, role);
        heading.paint = match subject.role {
            FaceRole::Diagnostic => GraphicsPaintRole::Warning,
            FaceRole::Status => GraphicsPaintRole::Foreground,
            FaceRole::Region | FaceRole::Collection => GraphicsPaintRole::Accent,
            _ => GraphicsPaintRole::Foreground,
        };
        append(heading);
        // The source-supplied Body status follows its name. This presents the
        // owner's exact wording; the Mask derives no lifecycle fact.
        {
            for text in face
                .text
                .iter()
                .filter(|text| text.subject == subject.identity)
            {
                append(item(
                    text.text.clone(),
                    if subject.role == FaceRole::Body {
                        GraphicsTextRole::Status
                    } else {
                        GraphicsTextRole::Body
                    },
                ));
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
            let available = action.availability.is_available();
            let checkbox = super::boolean_choice::selected(face, index);
            let label = if checkbox.is_some() {
                &subject.name
            } else {
                &action.name
            };
            let paint = if interaction_admitted && available {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
            append(Item {
                text: match &action.availability {
                    FaceActionAvailability::Unavailable {
                        reason_code,
                        explanation,
                    } => format!("{} · Unavailable: {} ({})", label, explanation, reason_code),
                    FaceActionAvailability::Refused {
                        reason_code,
                        explanation,
                    } => format!("{} · Refused: {} ({})", label, explanation, reason_code),
                    FaceActionAvailability::Available if !interaction_admitted => {
                        format!("{} · View only on this host", label)
                    }
                    FaceActionAvailability::Available => label.clone(),
                },
                role: GraphicsTextRole::Action,
                paint,
                control: interaction_admitted.then_some(FaceControl {
                    action: index,
                    argument: None,
                }),
                indent: 0,
                subject_index: None,
                region: None,
                checkbox,
            });
            if !available {
                continue;
            }
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                if checkbox.is_some() {
                    continue;
                }
                let hint = argument_hint(argument);
                append(Item {
                    text: super::text_field::current(face, index).map_or_else(
                        || format!("{} · {}", argument.value_name, hint),
                        |value| {
                            format!(
                                "{}: {}",
                                subject.name,
                                if value.is_empty() { "(empty)" } else { value }
                            )
                        },
                    ),
                    role: GraphicsTextRole::Label,
                    paint,
                    control: interaction_admitted.then_some(FaceControl {
                        action: index,
                        argument: Some(argument_index),
                    }),
                    indent: 0,
                    subject_index: None,
                    region: None,
                    checkbox: None,
                });
            }
        }
    }
    Ok((primary, details))
}

fn argument_hint(argument: &conduit_face::FaceActionArgument) -> String {
    if argument.contract.value_kind.as_str() == "value/bool" {
        return match (
            argument.contract.validate(&[0]).is_ok(),
            argument.contract.validate(&[1]).is_ok(),
        ) {
            (true, true) => "false or true",
            (true, false) => "false",
            (false, true) => "true",
            (false, false) => "No admitted Boolean value",
        }
        .into();
    }
    readable_finite_text_choices(&argument.contract).map_or_else(
        || "Enter a value".into(),
        |choices| format!("Choose {}", choices.join(", ")),
    )
}
