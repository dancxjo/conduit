#![no_std]

//! The Todo Plot's bounded semantic Face contribution. Masks decide how to
//! render it; this crate never stores Todo state or executes a user action.
//! Projection allocates while preparing a Face and is not a kernel Step Back.

extern crate alloc;

mod facts;
pub use facts::{TodoFaceFacts, TodoFactRefusal};

mod interaction;
pub use interaction::{todo_command_from_contributed_interaction, todo_command_from_interaction};

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_face::{
    FaceAction, FaceActionArgument, FaceActionAvailability, FaceContributionBasis, FaceDisclosure,
    FaceDisclosureLevel, FaceFragment, FaceFragmentError, FaceProperty, FacePropertyValue,
    FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};
use conduit_todo_plot::TodoState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoFaceError {
    InvalidState,
    InvalidActionContract,
    InvalidFragment(FaceFragmentError),
    InvalidInteraction(conduit_face::FaceInteractionRefusal),
    StaleState,
    UnsupportedAction,
    CommandRefused(conduit_todo_plot::TodoRefusal),
}

/// Project one validated Todo generation as Plot-owned Face truth. An owner
/// may advertise actions only after it has admitted an exact return route to
/// this Plot's current Play; a read-only route still exposes the same content.
pub fn todo_fragment(
    state: &TodoState,
    basis: FaceContributionBasis,
    actions_admitted: bool,
) -> Result<FaceFragment, TodoFaceError> {
    state.validate().map_err(|_| TodoFaceError::InvalidState)?;
    let list = "todo/list".to_string();
    let status = "todo/status".to_string();
    let open = state.items.iter().filter(|item| !item.complete).count();
    let completed = state.items.len() - open;
    let count = format!(
        "{open} {} left · {completed} completed",
        if open == 1 { "thing" } else { "things" }
    );
    let mut fragment = FaceFragment {
        basis,
        subjects: vec![
            FaceSubject {
                identity: list.clone(),
                role: FaceRole::Collection,
                name: state.title.clone(),
            },
            FaceSubject {
                identity: status.clone(),
                role: FaceRole::Status,
                name: "Progress".into(),
            },
        ],
        relationships: vec![FaceRelationship {
            source: list.clone(),
            target: status.clone(),
            kind: FaceRelationshipKind::Contains,
        }],
        composition: Vec::new(),
        properties: vec![
            FaceProperty {
                subject: list.clone(),
                name: "todo-revision".into(),
                value: FacePropertyValue::Count(u64::from(state.revision)),
            },
            // Global source facts remain available even when a Mask pages Items.
            FaceProperty {
                subject: list.clone(),
                name: "remaining-count".into(),
                value: FacePropertyValue::Count(open as u64),
            },
            FaceProperty {
                subject: list.clone(),
                name: "completed-count".into(),
                value: FacePropertyValue::Count(completed as u64),
            },
        ],
        text: vec![FaceText {
            subject: status.clone(),
            text: count,
        }],
        actions: Vec::new(),
        disclosures: vec![
            FaceDisclosure {
                subject: list.clone(),
                level: FaceDisclosureLevel::Primary,
            },
            FaceDisclosure {
                subject: status,
                level: FaceDisclosureLevel::Primary,
            },
        ],
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    let availability = if actions_admitted {
        FaceActionAvailability::Available
    } else {
        FaceActionAvailability::Unavailable {
            reason_code: "todo-return-route-unavailable".into(),
            explanation: "This Todo Play has no admitted action return route.".into(),
        }
    };
    let add_availability =
        if actions_admitted && state.items.len() == conduit_todo_plot::MAX_TODO_ITEMS {
            FaceActionAvailability::Unavailable {
                reason_code: "todo-list-full".into(),
                explanation: "The list has reached its admitted item capacity.".into(),
            }
        } else {
            availability.clone()
        };
    fragment.actions.push(FaceAction {
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
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: add_availability,
    });
    for (order, item) in state.items.iter().enumerate() {
        let identity = format!("todo/item/{}", item.id);
        fragment.subjects.push(FaceSubject {
            identity: identity.clone(),
            role: FaceRole::Item,
            name: item.text.clone(),
        });
        fragment.relationships.push(FaceRelationship {
            source: list.clone(),
            target: identity.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        fragment.properties.extend([
            FaceProperty {
                subject: identity.clone(),
                name: "complete".into(),
                value: FacePropertyValue::Flag(item.complete),
            },
            FaceProperty {
                subject: identity.clone(),
                name: "order".into(),
                value: FacePropertyValue::Count(order as u64),
            },
        ]);
        fragment.disclosures.push(FaceDisclosure {
            subject: identity.clone(),
            level: if item.complete {
                FaceDisclosureLevel::SelectedDetail
            } else {
                FaceDisclosureLevel::Primary
            },
        });
        fragment.actions.push(FaceAction {
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
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: availability.clone(),
        });
        fragment.actions.push(FaceAction {
            identity: format!("todo.remove.{}", item.id),
            intent: "todo/remove@1".into(),
            target: identity,
            name: "remove".into(),
            arguments: Vec::new(),
            disclosure: FaceDisclosureLevel::SelectedDetail,
            availability: availability.clone(),
        });
    }
    fragment
        .validate_bounds()
        .map_err(TodoFaceError::InvalidFragment)?;
    Ok(fragment)
}
