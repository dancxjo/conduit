//! Policy conformance against the actual Todo-owned Face projection.
use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
use conduit_presentation::*;
use conduit_std_host::spoken_face_mask::*;
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
fn todo_speech_default_is_selected_from_actual_plot_truth() {
    let face = todo(3, 20, true);
    let outline = select_spoken_outline(&face).unwrap();
    let opening = outline.clauses.join(" ");
    assert!(opening.starts_with("Courses & groceries. 3 things left · 17 completed"));
    assert!(opening.contains("Crème brûlée — 茶 🍵?"));
    assert!(opening.contains("Task 3."));
    assert!(!opening.contains("Task 4."));
    assert!(!opening.contains("task-1"));
    assert!(!opening.contains("todo-revision"));
    assert_eq!(outline.face_id, face.identity.as_str());
    assert_eq!(outline.face_revision, face.revision);
    assert_eq!(outline.action_ids, ["todo.complete.task-1"]);
    assert!(!opening.contains("add an item")); // The actual twenty-item list is full.
    let direct =
        conduit_std_host::direct_spoken_mask_runtime::prepare_wording_items(&face).unwrap();
    assert_eq!(
        std::str::from_utf8(direct.front().unwrap()).unwrap(),
        opening
    );
    let inspection = mechanical_face_clauses(&face).unwrap().join(" ");
    assert!(inspection.contains("Task 20"));
    assert!(inspection.contains("todo-revision"));
}

#[test]
fn todo_speech_pages_are_finite_complete_and_bound_to_one_face() {
    let face = todo(20, 20, true);
    let mut cursor = SpokenItemCursor::new(&face).unwrap();
    let mut ids = vec![];
    let mut pages = 0;
    loop {
        let page = cursor.next_page(&face).unwrap();
        assert!(page.subject_ids.len() <= 3);
        ids.extend(page.subject_ids);
        pages += 1;
        if !page.more {
            break;
        }
    }
    assert_eq!(pages, 7);
    assert_eq!(ids.len(), 20);
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        20
    );
    assert_eq!(
        ids,
        face.subjects
            .iter()
            .filter(|item| item.role == PresentationRole::Item)
            .map(|item| item.identity.clone())
            .collect::<Vec<_>>()
    );
    assert!(cursor.next_page(&face).unwrap().subject_ids.is_empty());
    let before = cursor.clone();
    assert_eq!(
        cursor.next_page(&todo(19, 20, true)),
        Err(SpokenFaceRefusal::StaleFace)
    );
    assert_eq!(cursor, before);
    let selected = todo(3, 20, true);
    let mut cursor = SpokenItemCursor::new(&selected).unwrap();
    assert_eq!(cursor.next_page(&selected).unwrap().subject_ids.len(), 3);
    assert!(cursor.next_page(&selected).unwrap().subject_ids.is_empty());
}

#[test]
fn todo_speech_empty_finished_and_unknown_detail_stay_truthful() {
    for total in [0, 20] {
        let face = todo(0, total, false);
        let outline = select_spoken_outline(&face).unwrap();
        assert!(outline
            .clauses
            .join(" ")
            .contains(&format!("0 things left · {total} completed")));
        assert!(outline.action_ids.is_empty());
        assert!(!outline.clauses.join(" ").contains("You can"));
        assert!(SpokenItemCursor::new(&face)
            .unwrap()
            .next_page(&face)
            .unwrap()
            .subject_ids
            .is_empty());
    }
    let base = todo(3, 20, true);
    let mut properties = base.properties.clone();
    properties.push(PresentationProperty {
        subject: "todo/list".into(),
        name: "future-field".into(),
        value: PresentationPropertyValue::Text("diagnostic-only".into()),
    });
    let face = Presentation::new_with_semantics(
        base.revision,
        base.basis.clone(),
        base.subjects.clone(),
        base.relationships.clone(),
        properties,
        base.text.clone(),
        base.actions.clone(),
        base.disclosures.clone(),
    )
    .unwrap();
    assert_eq!(
        select_spoken_outline(&face).unwrap().clauses,
        select_spoken_outline(&base).unwrap().clauses
    );
    assert!(mechanical_face_clauses(&face)
        .unwrap()
        .join(" ")
        .contains("diagnostic-only"));
}

#[test]
fn todo_speech_model_cannot_invent_completion_actions_or_selected_detail() {
    let face = todo(3, 20, true);
    let outline = select_spoken_outline(&face).unwrap();
    let index = outline.text_indices[0];
    let wording = &face.text[index as usize];
    let mut proposal = GeneratedWordingProposal {
        source_presentation_identity: face.identity.as_str().into(),
        source_presentation_revision: face.revision,
        clauses: vec![GeneratedWordingClause::Text {
            index,
            subject: wording.subject.clone(),
            value: wording.text.clone(),
            style: GeneratedWordingStyle::Direct,
        }],
    };
    assert!(outline.render_model_wording(&face, &proposal).is_ok());
    let GeneratedWordingClause::Text { value, .. } = &mut proposal.clauses[0] else {
        unreachable!()
    };
    *value = "All tasks completed".into();
    assert_eq!(
        outline.render_model_wording(&face, &proposal),
        Err(GeneratedWordingRefusal::InventedClaim)
    );
    proposal.clauses = vec![GeneratedWordingClause::Action {
        index: 0,
        identity: "todo.launch-rocket".into(),
        name: "Launch rocket".into(),
        style: GeneratedWordingStyle::Direct,
    }];
    assert_eq!(
        outline.render_model_wording(&face, &proposal),
        Err(GeneratedWordingRefusal::InventedClaim)
    );
    let (index, action) = face
        .actions
        .iter()
        .enumerate()
        .find(|(_, action)| action.identity == "todo.complete.task-2")
        .unwrap();
    proposal.clauses = vec![GeneratedWordingClause::Action {
        index: index as u32,
        identity: action.identity.clone(),
        name: action.name.clone(),
        style: GeneratedWordingStyle::Direct,
    }];
    assert!(proposal.render_exact(&face).is_ok());
    assert_eq!(
        outline.render_model_wording(&face, &proposal),
        Err(GeneratedWordingRefusal::InventedClaim)
    );
    proposal.source_presentation_revision += 1;
    assert_eq!(
        outline.render_model_wording(&face, &proposal),
        Err(GeneratedWordingRefusal::StaleFace)
    );
}
