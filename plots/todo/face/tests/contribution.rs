use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
use conduit_face::{
    Face, FaceActionAvailability, FaceBasis, FaceContributionBasis, FaceDisclosureLevel,
    FacePropertyValue, FaceRole,
};
use conduit_todo_face::{todo_fragment, TodoFaceError};
use conduit_todo_plot::{TodoItem, TodoState};

fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
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
        .find(|subject| subject.role == FaceRole::Collection)
        .unwrap();
    assert_eq!(list.name, "Groceries");
    assert!(!fragment
        .text
        .iter()
        .any(|wording| wording.text == list.name));
    assert!(fragment.text.iter().any(|wording| {
        wording.subject == "todo/status" && wording.text == "3 things left · 17 completed"
    }));
    assert_eq!(
        fragment
            .disclosures
            .iter()
            .find(|entry| entry.subject == list.identity)
            .unwrap()
            .level,
        FaceDisclosureLevel::Primary
    );
    let primary = fragment
        .subjects
        .iter()
        .filter(|subject| {
            subject.role == FaceRole::Item
                && fragment.disclosures.iter().any(|entry| {
                    entry.subject == subject.identity && entry.level == FaceDisclosureLevel::Primary
                })
        })
        .count();
    assert_eq!(primary, 3);
    assert_eq!(
        fragment
            .disclosures
            .iter()
            .filter(|entry| entry.level == FaceDisclosureLevel::SelectedDetail)
            .count(),
        17
    );
    assert!(fragment.properties.iter().any(|property| {
        property.subject == "todo/item/task-4"
            && property.name == "complete"
            && property.value == FacePropertyValue::Flag(true)
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.add"
            && action.target == "todo/list"
            && action.arguments.len() == 1
            && matches!(&action.availability, FaceActionAvailability::Unavailable { reason_code, .. } if reason_code == "todo-list-full")
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.complete.task-1"
            && action.target == "todo/item/task-1"
            && action.availability == FaceActionAvailability::Available
    }));
    assert!(fragment.actions.iter().any(|action| {
        action.identity == "todo.reopen.task-4" && action.target == "todo/item/task-4"
    }));
    let face = Face::new_with_semantics(
        20,
        FaceBasis {
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
            FaceActionAvailability::Unavailable { .. }
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

#[test]
fn source_count_facts_cover_the_whole_generation_independent_of_item_disclosure() {
    for (remaining, total) in [(0, 0), (1, 1), (3, 3), (3, 20), (20, 20), (0, 20)] {
        let state = TodoState {
            title: "Courses & groceries".into(),
            revision: 42,
            next_id: total as u32 + 1,
            items: (0..total)
                .map(|index| TodoItem {
                    id: format!("task-{}", index + 1),
                    text: if index == 0 {
                        "Crème brûlée — 茶 🍵?".into()
                    } else {
                        format!("Task {}", index + 1)
                    },
                    complete: index >= remaining,
                })
                .collect(),
        };
        let original = state.clone();
        for admitted in [false, true] {
            let fragment = todo_fragment(&state, basis(), admitted).unwrap();
            let count = |name: &str| {
                let facts: Vec<_> = fragment
                    .properties
                    .iter()
                    .filter(|p| p.subject == "todo/list" && p.name == name)
                    .collect();
                assert_eq!(facts.len(), 1);
                let FacePropertyValue::Count(value) = facts[0].value else {
                    panic!("source count must be typed")
                };
                value
            };
            assert_eq!(count("remaining-count"), remaining as u64);
            assert_eq!(count("completed-count"), (total - remaining) as u64);
            assert_eq!(count("todo-revision"), 42);
            assert_eq!(
                fragment
                    .subjects
                    .iter()
                    .filter(|s| s.role == FaceRole::Item)
                    .count(),
                total
            );
            assert!(fragment.actions.iter().any(|a| a.identity == "todo.add"));
            assert_eq!(state, original);
        }
    }
}
