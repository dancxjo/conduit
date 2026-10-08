#![no_std]

//! The Todo Plot's bounded semantic Face contribution. Masks decide how to
//! render it; this crate never stores Todo state or executes a user action.
//! Projection allocates while preparing a Face and is not a kernel Step Back.

extern crate alloc;

mod interaction;
pub use interaction::todo_command_from_interaction;

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_presentation::{
    FaceActionArgument, PresentationAction, PresentationActionAvailability,
    PresentationContributionBasis, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationFragment, PresentationFragmentError, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};
use conduit_todo_plot::TodoState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoFaceError {
    InvalidState,
    InvalidActionContract,
    InvalidFragment(PresentationFragmentError),
    InvalidInteraction(conduit_presentation::FaceInteractionRefusal),
    StaleState,
    UnsupportedAction,
    CommandRefused(conduit_todo_plot::TodoRefusal),
}

/// Project one validated Todo generation as Plot-owned Face truth. An owner
/// may advertise actions only after it has admitted an exact return route to
/// this Plot's current Play; a read-only route still exposes the same content.
pub fn todo_fragment(
    state: &TodoState,
    basis: PresentationContributionBasis,
    actions_admitted: bool,
) -> Result<PresentationFragment, TodoFaceError> {
    state.validate().map_err(|_| TodoFaceError::InvalidState)?;
    let list = "todo/list".to_string();
    let status = "todo/status".to_string();
    let open = state.items.iter().filter(|item| !item.complete).count();
    let completed = state.items.len() - open;
    let count = format!(
        "{open} {} left · {completed} completed",
        if open == 1 { "thing" } else { "things" }
    );
    let mut fragment = PresentationFragment {
        basis,
        subjects: vec![
            PresentationSubject {
                identity: list.clone(),
                role: PresentationRole::Collection,
                name: state.title.clone(),
            },
            PresentationSubject {
                identity: status.clone(),
                role: PresentationRole::Status,
                name: count.clone(),
            },
        ],
        relationships: vec![PresentationRelationship {
            source: list.clone(),
            target: status.clone(),
            kind: PresentationRelationshipKind::Contains,
        }],
        composition: Vec::new(),
        properties: vec![PresentationProperty {
            subject: list.clone(),
            name: "todo-revision".into(),
            value: PresentationPropertyValue::Count(u64::from(state.revision)),
        }],
        text: vec![
            PresentationText {
                subject: list.clone(),
                text: state.title.clone(),
            },
            PresentationText {
                subject: status.clone(),
                text: count,
            },
        ],
        actions: Vec::new(),
        disclosures: vec![
            PresentationDisclosure {
                subject: list.clone(),
                level: PresentationDisclosureLevel::Context,
            },
            PresentationDisclosure {
                subject: status,
                level: PresentationDisclosureLevel::Primary,
            },
        ],
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    let availability = if actions_admitted {
        PresentationActionAvailability::Available
    } else {
        PresentationActionAvailability::Unavailable {
            reason_code: "todo-return-route-unavailable".into(),
            explanation: "This Todo Play has no admitted action return route.".into(),
        }
    };
    let add_availability =
        if actions_admitted && state.items.len() == conduit_todo_plot::MAX_TODO_ITEMS {
            PresentationActionAvailability::Unavailable {
                reason_code: "todo-list-full".into(),
                explanation: "The list has reached its admitted item capacity.".into(),
            }
        } else {
            availability.clone()
        };
    fragment.actions.push(PresentationAction {
        identity: "todo.add".into(),
        intent: "todo/add@1".into(),
        target: list.clone(),
        name: "add an item".into(),
        arguments: vec![FaceActionArgument::text(
            "text".into(),
            "Item text".into(),
            1,
            conduit_todo_plot::MAX_TODO_TEXT_BYTES as u32,
        )
        .map_err(|_| TodoFaceError::InvalidActionContract)?],
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: add_availability,
    });
    for (order, item) in state.items.iter().enumerate() {
        let identity = format!("todo/item/{}", item.id);
        fragment.subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: item.text.clone(),
        });
        fragment.relationships.push(PresentationRelationship {
            source: list.clone(),
            target: identity.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        fragment.properties.extend([
            PresentationProperty {
                subject: identity.clone(),
                name: "complete".into(),
                value: PresentationPropertyValue::Flag(item.complete),
            },
            PresentationProperty {
                subject: identity.clone(),
                name: "order".into(),
                value: PresentationPropertyValue::Count(order as u64),
            },
        ]);
        fragment.disclosures.push(PresentationDisclosure {
            subject: identity.clone(),
            level: if item.complete {
                PresentationDisclosureLevel::SelectedDetail
            } else {
                PresentationDisclosureLevel::Primary
            },
        });
        fragment.actions.push(PresentationAction {
            identity: format!(
                "todo.{}.{id}",
                if item.complete { "reopen" } else { "complete" },
                id = item.id
            ),
            intent: if item.complete {
                "todo/reopen@1"
            } else {
                "todo/complete@1"
            }
            .into(),
            target: identity.clone(),
            name: if item.complete { "reopen" } else { "complete" }.into(),
            arguments: Vec::new(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: availability.clone(),
        });
        fragment.actions.push(PresentationAction {
            identity: format!("todo.remove.{}", item.id),
            intent: "todo/remove@1".into(),
            target: identity,
            name: "remove".into(),
            arguments: Vec::new(),
            disclosure: PresentationDisclosureLevel::SelectedDetail,
            availability: availability.clone(),
        });
    }
    fragment
        .validate_bounds()
        .map_err(TodoFaceError::InvalidFragment)?;
    Ok(fragment)
}
