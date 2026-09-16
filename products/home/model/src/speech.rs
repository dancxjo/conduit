use alloc::{format, string::String, vec::Vec};
use conduit_presentation::{ApplicationComponent, ApplicationNodeState, ApplicationView};

pub const MAX_AURAL_UTTERANCES: usize = 32;
pub const MAX_AURAL_UTTERANCE_BYTES: usize = 256;
pub const MAX_AURAL_PRESENTATION_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuralRole {
    Heading,
    Content,
    Action,
    Status,
    Code,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuralUtterance {
    pub subject_key: String,
    pub role: AuralRole,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuralPresentation {
    pub source_revision: u32,
    pub utterances: Vec<AuralUtterance>,
    pub total_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuralRefusal {
    InvalidView,
    StalePreviousView,
    TooManyUtterances,
    UtteranceTooLong,
    PresentationTooLong,
}

pub fn linearize(
    current: &ApplicationView,
    previous: Option<&ApplicationView>,
) -> Result<AuralPresentation, AuralRefusal> {
    current.validate().map_err(|_| AuralRefusal::InvalidView)?;
    if previous.is_some_and(|previous| {
        previous.validate().is_err() || previous.revision >= current.revision
    }) {
        return Err(AuralRefusal::StalePreviousView);
    }
    let mut utterances = Vec::new();
    let mut total_bytes = 0_usize;
    for node in &current.nodes {
        if previous.is_some_and(|previous| {
            previous
                .nodes
                .iter()
                .find(|candidate| candidate.key == node.key)
                .is_some_and(|candidate| {
                    candidate.component == node.component
                        && candidate.text == node.text
                        && candidate.value == node.value
                        && candidate.state == node.state
                })
        }) {
            continue;
        }
        let Some(role) = aural_role(node.component) else {
            continue;
        };
        let text = spoken_text(&node.text, &node.value, node.state);
        if text.is_empty() {
            continue;
        }
        if text.len() > MAX_AURAL_UTTERANCE_BYTES {
            return Err(AuralRefusal::UtteranceTooLong);
        }
        total_bytes = total_bytes
            .checked_add(text.len())
            .ok_or(AuralRefusal::PresentationTooLong)?;
        if total_bytes > MAX_AURAL_PRESENTATION_BYTES {
            return Err(AuralRefusal::PresentationTooLong);
        }
        if utterances.len() == MAX_AURAL_UTTERANCES {
            return Err(AuralRefusal::TooManyUtterances);
        }
        utterances.push(AuralUtterance {
            subject_key: node.key.clone(),
            role,
            text,
        });
    }
    Ok(AuralPresentation {
        source_revision: current.revision,
        utterances,
        total_bytes,
    })
}

fn aural_role(component: ApplicationComponent) -> Option<AuralRole> {
    match component {
        ApplicationComponent::Panel | ApplicationComponent::Heading => Some(AuralRole::Heading),
        ApplicationComponent::Paragraph
        | ApplicationComponent::Summary
        | ApplicationComponent::Definition => Some(AuralRole::Content),
        ApplicationComponent::Button
        | ApplicationComponent::NavigationLink
        | ApplicationComponent::Link => Some(AuralRole::Action),
        ApplicationComponent::Status
        | ApplicationComponent::SuccessStatus
        | ApplicationComponent::FailureStatus
        | ApplicationComponent::WarningStatus
        | ApplicationComponent::MissingEvidence
        | ApplicationComponent::StaleEvidence
        | ApplicationComponent::RefusedEvidence
        | ApplicationComponent::FailedEvidence
        | ApplicationComponent::SuccessfulEvidence => Some(AuralRole::Status),
        ApplicationComponent::Code | ApplicationComponent::CodeBlock => Some(AuralRole::Code),
        _ => None,
    }
}

fn spoken_text(text: &str, value: &str, state: ApplicationNodeState) -> String {
    let content = match (text.trim(), value.trim()) {
        ("", "") => return String::new(),
        ("", value) => value.into(),
        (text, "") => text.into(),
        (text, value) if text == value => text.into(),
        (text, value) => format!("{text}. {value}"),
    };
    match state {
        ApplicationNodeState::Ready => content,
        ApplicationNodeState::Busy => format!("{content}. Busy."),
        ApplicationNodeState::Unavailable => format!("{content}. Unavailable."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HomeEvent, HomeModel};

    const FORMS: [&str; 2] = ["Hello", "Patchbay"];

    #[test]
    fn home_is_linearized_from_semantics_without_pixel_facts() {
        let home = HomeModel::new();
        let view = home.presentation(1, &FORMS).lower().unwrap();
        let spoken = linearize(&view, None).unwrap();
        let text: Vec<_> = spoken
            .utterances
            .iter()
            .map(|utterance| utterance.text.as_str())
            .collect();
        assert!(text.contains(&"Conduit Home"));
        assert!(text.contains(&"TOUR"));
        assert!(text.contains(&"PROMPT"));
        assert!(text.iter().all(|value| !value.contains("pixel")));
    }

    #[test]
    fn revision_delta_speaks_only_changed_semantic_subjects() {
        let mut home = HomeModel::new();
        let before = home.presentation(1, &FORMS).lower().unwrap();
        home.accept(HomeEvent::Text("help"), &FORMS);
        let after = home.presentation(2, &FORMS).lower().unwrap();
        let spoken = linearize(&after, Some(&before)).unwrap();
        assert!(spoken.utterances.iter().any(|utterance| {
            utterance.subject_key == "command" && utterance.text.contains("help")
        }));
        assert!(
            spoken
                .utterances
                .iter()
                .all(|utterance| utterance.subject_key != "application-0")
        );
    }

    #[test]
    fn stale_delta_basis_is_refused() {
        let home = HomeModel::new();
        let view = home.presentation(2, &FORMS).lower().unwrap();
        assert_eq!(
            linearize(&view, Some(&view)),
            Err(AuralRefusal::StalePreviousView)
        );
    }
}
