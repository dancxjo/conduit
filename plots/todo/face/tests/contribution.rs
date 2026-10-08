use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
use conduit_presentation::{
    Presentation, PresentationActionAvailability, PresentationBasis, PresentationContributionBasis,
    PresentationDisclosureLevel, PresentationPropertyValue, PresentationRole,
};
use conduit_todo_face::{todo_fragment, TodoFaceError};
use conduit_todo_plot::{TodoItem, TodoState};

fn basis() -> PresentationContributionBasis {
    PresentationContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/todo"),
        plan_id: PlanId::from("plan/todo"),
        active_play_id: ActivePlayId::from("play/todo"),
        required_interaction_context: None,
    }
}

fn twenty_items() -> TodoState {
    TodoState {
        title: "Groceries".into(),
        revision: 20,
        next_id: 21,
        items: (1..=20)
            .map(|number| TodoItem {
                id: format!("task-{number}"),
                text: format!("Task {number}"),
                complete: number > 3,
            })
            .collect(),
    }
}

#[test]
fn same_todo_truth_is_ready_for_distinct_masks_without_reading_seventeen_completed_items() {
    let state = twenty_items();
    let fragment = todo_fragment(&state, basis(), true).unwrap();
    let list = fragment
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Collection)
        .unwrap();
    assert_eq!(list.name, "Groceries");
    assert_eq!(
        fragment
            .disclosures
            .iter()
            .find(|entry| entry.subject == list.identity)
            .unwrap()
            .level,
        PresentationDisclosureLevel::Context
    );
    let primary = fragment
        .subjects
        .iter()
        .filter(|subject| {
            subject.role == PresentationRole::Item
                && fragment.disclosures.iter().any(|entry| {
                    entry.subject == subject.identity
                        && entry.level == PresentationDisclosureLevel::Primary
                })
        })
        .count();
    assert_eq!(primary, 3);
    assert_eq!(
        fragment
            .disclosures
            .iter()
            .filter(|entry| entry.level == PresentationDisclosureLevel::SelectedDetail)
            .count(),
        17
    );
    assert!(fragment.properties.iter().any(|property| {
        property.subject == "todo/item/task-4"
            && property.name == "complete"
            && property.value == PresentationPropertyValue::Flag(true)
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.add"
            && action.target == "todo/list"
            && action.arguments.len() == 1
            && action.availability == PresentationActionAvailability::Available
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.complete.task-1" && action.target == "todo/item/task-1"
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.reopen.task-4" && action.target == "todo/item/task-4"
    }));
    let face = Presentation::new_with_semantics(
        20,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/todo")),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/todo")),
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
    .unwrap();
    face.validate().unwrap();
}

#[test]
fn read_only_projection_cannot_advertise_an_unadmitted_action_route() {
    let fragment = todo_fragment(&twenty_items(), basis(), false).unwrap();
    assert!(fragment.actions.iter().all(|action| {
        matches!(
            action.availability,
            PresentationActionAvailability::Unavailable { .. }
        )
    }));
}

#[test]
fn malformed_todo_truth_is_refused_before_face_projection() {
    let mut state = twenty_items();
    state.items[1].id = state.items[0].id.clone();
    assert_eq!(
        todo_fragment(&state, basis(), true),
        Err(TodoFaceError::InvalidState)
    );
}
