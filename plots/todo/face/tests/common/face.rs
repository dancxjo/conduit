#![allow(dead_code)]
use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};
use conduit_face::*;
use conduit_todo_face::todo_fragment;
use conduit_todo_plot::{TodoItem, TodoState};
pub fn face(remaining: usize, total: usize, admitted: bool) -> Face {
    let state = TodoState {
        title: "Groceries".into(),
        revision: 7,
        next_id: total as u32 + 1,
        items: (0..total)
            .map(|i| TodoItem {
                id: format!("task-{}", i + 1),
                text: if i == 0 {
                    "Crème brûlée — 茶 🍵?".into()
                } else {
                    format!("Task {}", i + 1)
                },
                complete: i >= remaining,
            })
            .collect(),
    };
    let fragment = todo_fragment(
        &state,
        FaceContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/todo"),
            plan_id: PlanId::from("plan/todo"),
            active_play_id: ActivePlayId::from("play/todo"),
            required_interaction_context: None,
        },
        admitted,
    )
    .unwrap();
    Face::new_with_semantics(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(conduit_core::SourceDocumentId::from("source/todo")),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some(conduit_core::ExpandedPlotId::from("expanded/todo")),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap()
}
pub fn rehash(value: Face) -> Face {
    Face::new_with_semantics(
        value.revision,
        value.basis,
        value.subjects,
        value.relationships,
        value.properties,
        value.text,
        value.actions,
        value.disclosures,
    )
    .unwrap()
}
