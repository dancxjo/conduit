use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};
use conduit_face::*;
use conduit_todo_face::{todo_fragment, TodoFaceFacts, TodoFactRefusal};
use conduit_todo_plot::{TodoItem, TodoState};
fn face(remaining: usize, total: usize, admitted: bool) -> Face {
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
fn rehash(value: Face) -> Face {
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
#[test]
fn exact_source_facts_and_names_survive_a_page_without_recounting_it() {
    for (remaining, total) in [(0, 0), (1, 1), (3, 3), (3, 20), (20, 20), (0, 20)] {
        let original = face(remaining, total, true);
        let facts = TodoFaceFacts::prepare(&original).unwrap();
        assert!(core::ptr::eq(facts.source(), &original));
        assert_eq!(facts.remaining(), remaining as u64);
        assert_eq!(facts.completed(), (total - remaining) as u64);
        let ids: Vec<String> = (0..remaining.min(3))
            .map(|i| format!("todo/item/task-{}", i + 1))
            .collect();
        let refs: Vec<_> = ids.iter().map(String::as_str).collect();
        let page = facts.remaining_page(&refs).unwrap();
        assert_eq!(page.len(), remaining.min(3));
        assert_eq!(facts.remaining(), remaining as u64);
        if remaining > 0 {
            assert_eq!(page[0].name, "Crème brûlée — 茶 🍵?");
            let action = facts.action(&original, "todo.complete.task-1").unwrap();
            assert_eq!(action.target, page[0].identity);
            assert!(core::ptr::eq(
                action,
                original
                    .actions
                    .iter()
                    .find(|a| a.identity == action.identity)
                    .unwrap()
            ));
        }
    }
}
#[test]
fn missing_wrong_typed_and_out_of_bounds_source_facts_refuse() {
    let original = face(1, 1, true);
    let mut missing = original.clone();
    missing.properties.retain(|p| p.name != "remaining-count");
    assert!(matches!(
        TodoFaceFacts::prepare(&rehash(missing)),
        Err(TodoFactRefusal::Missing)
    ));
    let mut wrong = original.clone();
    wrong
        .properties
        .iter_mut()
        .find(|p| p.name == "remaining-count")
        .unwrap()
        .value = FacePropertyValue::Text("1".into());
    assert!(matches!(
        TodoFaceFacts::prepare(&rehash(wrong)),
        Err(TodoFactRefusal::Type)
    ));
    let mut large = original.clone();
    large
        .properties
        .iter_mut()
        .find(|p| p.name == "remaining-count")
        .unwrap()
        .value = FacePropertyValue::Count(21);
    assert!(matches!(
        TodoFaceFacts::prepare(&rehash(large)),
        Err(TodoFactRefusal::Bound)
    ));
    let mut corrupt = original;
    corrupt
        .properties
        .iter_mut()
        .find(|p| p.name == "remaining-count")
        .unwrap()
        .value = FacePropertyValue::Count(0);
    assert!(matches!(
        TodoFaceFacts::prepare(&corrupt),
        Err(TodoFactRefusal::Face)
    ));
}
#[test]
fn stale_actions_undisclosed_items_invented_ids_and_page_pressure_refuse() {
    let original = face(20, 20, true);
    let facts = TodoFaceFacts::prepare(&original).unwrap();
    let mut next = original.clone();
    next.revision += 1;
    let next = rehash(next);
    assert!(matches!(
        facts.action(&next, "todo.complete.task-1"),
        Err(TodoFactRefusal::Stale)
    ));
    assert!(facts.action(&original, "invented.action").is_err());
    assert!(facts.remaining_item("foreign/item").is_err());
    assert!(matches!(
        facts.remaining_page(&["todo/item/task-1", "todo/item/task-1"]),
        Err(TodoFactRefusal::Duplicate)
    ));
    assert!(matches!(
        facts.remaining_page(&[
            "todo/item/task-1",
            "todo/item/task-2",
            "todo/item/task-3",
            "todo/item/task-4"
        ]),
        Err(TodoFactRefusal::Bound)
    ));
    assert!(matches!(
        facts.remaining_page(&["todo/item/task-2", "todo/item/task-1"]),
        Err(TodoFactRefusal::Item)
    ));
    let mut hidden = original.clone();
    hidden
        .disclosures
        .iter_mut()
        .find(|d| d.subject == "todo/item/task-1")
        .unwrap()
        .level = FaceDisclosureLevel::SelectedDetail;
    let hidden = rehash(hidden);
    let hidden = TodoFaceFacts::prepare(&hidden).unwrap();
    assert!(matches!(
        hidden.remaining_item("todo/item/task-1"),
        Err(TodoFactRefusal::Disclosure)
    ));
    let readonly = face(1, 1, false);
    let readonly_facts = TodoFaceFacts::prepare(&readonly).unwrap();
    assert_eq!(readonly_facts.remaining(), 1);
    assert!(matches!(
        readonly_facts.action(&readonly, "todo.complete.task-1"),
        Err(TodoFactRefusal::Action)
    ));
}
