//! Terminal selection against the actual Todo Face; Show is a presentation fixture.
use super::*;
use crate::mask_test_common as common;
use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
use conduit_presentation::*;
use conduit_todo_plot::{TodoItem, TodoState};

fn todo(open: usize, total: usize, actions: bool) -> Presentation {
    let state = TodoState {
        title: "Courses & groceries".into(),
        revision: total as u32,
        next_id: total as u32 + 1,
        items: (0..total)
            .map(|index| TodoItem {
                id: format!("task-{}", index + 1),
                text: if index == 0 {
                    "Crème brûlée — 茶 🍵?".into()
                } else {
                    format!("Task {}", index + 1)
                },
                complete: index >= open,
            })
            .collect(),
    };
    let fragment = conduit_todo_face::todo_fragment(
        &state,
        PresentationContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/todo"),
            plan_id: PlanId::from("plan/todo"),
            active_play_id: ActivePlayId::from("play/todo"),
            required_interaction_context: None,
        },
        actions,
    )
    .unwrap();
    Presentation::new_with_semantics(
        u64::from(state.revision),
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
    .unwrap()
}

#[test]
fn named_todo_actions_keep_exact_ids_targets_and_arguments() {
    let face = todo(2, 3, true);
    let show = common::available_mask_show(&face);
    for id in [
        "todo.complete.task-1",
        "todo.reopen.task-3",
        "todo.remove.task-2",
    ] {
        let interaction = interaction_for_action(&face, &show, Some(id), "").unwrap();
        assert_eq!(interaction.action_id, id);
        assert!(interaction.arguments.is_empty());
        assert_eq!(interaction.show_id, show.show_id.as_str());
        interaction.validate_against(&face, &show).unwrap();
    }
    let add = interaction_for_action(&face, &show, Some("todo.add"), "Tea 茶").unwrap();
    assert_eq!(add.arguments.len(), 1);
    assert_eq!(add.arguments[0].value, "Tea 茶".as_bytes());
    assert!(interaction_for_action(&face, &show, None, "Tea").is_err());
    assert!(interaction_for_action(
        &face,
        &show,
        Some("todo.add"),
        &"a".repeat(conduit_todo_plot::MAX_TODO_TEXT_BYTES + 1)
    )
    .is_err());
    assert!(interaction_for_action(&face, &show, Some("todo.complete.task-1"), "extra").is_err());
    assert!(interaction_for_action(&face, &show, Some("invented"), "").is_err());
}

#[test]
fn terminal_action_refuses_unavailable_and_stale_show() {
    let full = todo(20, 20, true);
    let show = common::available_mask_show(&full);
    assert!(interaction_for_action(&full, &show, Some("todo.add"), "Overflow").is_err());
    let changed = todo(19, 20, true);
    assert!(interaction_for_action(&changed, &show, Some("todo.complete.task-1"), "").is_err());
}

#[test]
fn terminal_action_help_names_actual_targets_and_values() {
    let face = todo(2, 3, true);
    let mut output = Vec::new();
    report_actions(&mut output, &face).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("complete — Crème brûlée — 茶 🍵?: action todo.complete.task-1"));
    assert!(output.contains("reopen — Task 3: action todo.reopen.task-3"));
    assert!(output.contains("action todo.add <text>"));
}

#[test]
fn requested_show_evidence_retains_exact_revision_and_refuses_a_foreign_face() {
    let mut face = todo(2, 3, true);
    face = Presentation::new_with_semantics(
        u64::MAX,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let evidence = evidence_for_show(&face, &show).unwrap();
    assert_eq!(evidence["face_revision"], "18446744073709551615");
    assert_eq!(evidence["face_id"], face.identity.as_str());
    assert_eq!(evidence["show_id"], show.show_id.as_str());
    assert_eq!(evidence["mask_play_id"], show.show.active_play_id.as_str());
    assert!(evidence_for_show(&todo(1, 3, true), &show).is_err());
}
