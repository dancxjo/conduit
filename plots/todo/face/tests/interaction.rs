use conduit_core::{
    kind_id, ActivePlayId, CheckedPlotId, CheckedValueContract, ExpandedPlotId, PlanId,
    SourceDocumentId,
};
use conduit_presentation::{
    FaceActionArgument, FaceInteraction, FaceInteractionArgument, FaceInteractionRefusal, MaskShow,
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationContributionBasis, PresentationRole, PresentationSubject, UTF8_TEXT_VALUE_KIND,
};
use conduit_todo_face::{todo_command_from_interaction, todo_fragment, TodoFaceError};
use conduit_todo_plot::{TodoCommand, TodoItem, TodoState};

#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod common;

fn state() -> TodoState {
    TodoState {
        title: "Groceries".into(),
        revision: 2,
        next_id: 3,
        items: vec![
            TodoItem {
                id: "task-1".into(),
                text: "Milk".into(),
                complete: false,
            },
            TodoItem {
                id: "task-2".into(),
                text: "Bread".into(),
                complete: true,
            },
        ],
    }
}

fn todo_face(state: &TodoState, actions_admitted: bool) -> (Presentation, MaskShow) {
    let basis = PresentationContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/todo"),
        plan_id: PlanId::from("plan/todo"),
        active_play_id: ActivePlayId::from("play/todo"),
        required_interaction_context: None,
    };
    let fragment = todo_fragment(state, basis.clone(), actions_admitted).unwrap();
    let mut subjects = fragment.subjects;
    subjects.push(PresentationSubject {
        identity: "patchbay/plot".into(),
        role: PresentationRole::Plot,
        name: "Todo encounter".into(),
    });
    let face = Presentation::new_with_semantics(
        u64::from(state.revision),
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/todo")),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/todo")),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    (face, show)
}

fn reface(face: Presentation, actions: Vec<PresentationAction>) -> (Presentation, MaskShow) {
    let revised = Presentation::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        actions,
        face.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&revised);
    (revised, show)
}

fn interaction(
    face: &Presentation,
    show: &MaskShow,
    action: &str,
    target: &str,
    arguments: Vec<FaceInteractionArgument>,
) -> FaceInteraction {
    FaceInteraction::new(face, show, action, target, arguments, 1).unwrap()
}

#[test]
fn every_todo_action_routes_to_one_typed_command_without_changing_state() {
    let state = state();
    let original = state.clone();
    let (face, show) = todo_face(&state, true);
    for (action, target, arguments, expected) in [
        (
            "todo.add",
            "todo/list",
            vec![FaceInteractionArgument {
                name: "text".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: b"Tea".to_vec(),
            }],
            TodoCommand::Add { text: "Tea".into() },
        ),
        (
            "todo.complete.task-1",
            "todo/item/task-1",
            vec![],
            TodoCommand::SetComplete {
                id: "task-1".into(),
                complete: true,
            },
        ),
        (
            "todo.reopen.task-2",
            "todo/item/task-2",
            vec![],
            TodoCommand::SetComplete {
                id: "task-2".into(),
                complete: false,
            },
        ),
        (
            "todo.remove.task-1",
            "todo/item/task-1",
            vec![],
            TodoCommand::Remove {
                id: "task-1".into(),
            },
        ),
    ] {
        let interaction = interaction(&face, &show, action, target, arguments);
        let command = todo_command_from_interaction(&state, &face, &show, &interaction).unwrap();
        assert_eq!(command, expected);
        assert_eq!(
            TodoCommand::decode_info(&command.encode_info().unwrap()).unwrap(),
            command
        );
    }
    assert_eq!(state, original);
}

#[test]
fn stale_unknown_unavailable_and_malformed_interactions_refuse() {
    let state = state();
    let (face, show) = todo_face(&state, true);
    let add = interaction(
        &face,
        &show,
        "todo.add",
        "todo/list",
        vec![FaceInteractionArgument {
            name: "text".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: b"Tea".to_vec(),
        }],
    );
    let later = state
        .apply(&TodoCommand::Add {
            text: "Eggs".into(),
        })
        .unwrap();
    assert_eq!(
        todo_command_from_interaction(&later, &face, &show, &add),
        Err(TodoFaceError::StaleState)
    );
    let (later_face, later_show) = todo_face(&later, true);
    let later_add = interaction(
        &later_face,
        &later_show,
        "todo.add",
        "todo/list",
        add.arguments.clone(),
    );
    assert_eq!(
        todo_command_from_interaction(&state, &face, &show, &later_add),
        Err(TodoFaceError::InvalidInteraction(
            FaceInteractionRefusal::StaleFace
        ))
    );

    let mut wrong_kind = add.clone();
    wrong_kind.arguments[0].value_kind = "value/bool".into();
    assert_eq!(
        todo_command_from_interaction(&state, &face, &show, &wrong_kind),
        Err(TodoFaceError::InvalidInteraction(
            FaceInteractionRefusal::WrongValueKind
        ))
    );
    let mut extra = add.clone();
    extra.arguments.push(FaceInteractionArgument {
        name: "extra".into(),
        value_kind: UTF8_TEXT_VALUE_KIND.into(),
        value: b"x".to_vec(),
    });
    assert_eq!(
        todo_command_from_interaction(&state, &face, &show, &extra),
        Err(TodoFaceError::InvalidInteraction(
            FaceInteractionRefusal::UnknownArgument
        ))
    );
    let mut malformed_text = add.clone();
    malformed_text.arguments[0].value = vec![0xff];
    assert_eq!(
        todo_command_from_interaction(&state, &face, &show, &malformed_text),
        Err(TodoFaceError::InvalidInteraction(
            FaceInteractionRefusal::MalformedEncoding
        ))
    );

    let (read_only, read_only_show) = todo_face(&state, false);
    assert_eq!(
        FaceInteraction::new(
            &read_only,
            &read_only_show,
            "todo.add",
            "todo/list",
            add.arguments.clone(),
            2,
        ),
        Err(FaceInteractionRefusal::UnavailableAction)
    );

    let mut actions = face.actions.clone();
    actions.push(PresentationAction {
        identity: "todo.unknown".into(),
        intent: "todo/unknown@1".into(),
        target: "todo/list".into(),
        name: "unknown".into(),
        arguments: vec![],
        disclosure: conduit_presentation::PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    });
    let (forged, forged_show) = reface(face.clone(), actions);
    let unknown = interaction(&forged, &forged_show, "todo.unknown", "todo/list", vec![]);
    assert_eq!(
        todo_command_from_interaction(&state, &forged, &forged_show, &unknown),
        Err(TodoFaceError::UnsupportedAction)
    );

    let mut actions = face.actions.clone();
    let add_action = actions
        .iter_mut()
        .find(|item| item.identity == "todo.add")
        .unwrap();
    add_action.arguments = vec![FaceActionArgument {
        name: "text".into(),
        value_name: "Text".into(),
        contract: CheckedValueContract::new(kind_id("value/bool"), 1, vec![]).unwrap(),
    }];
    let (forged, forged_show) = reface(face, actions);
    let malformed = interaction(
        &forged,
        &forged_show,
        "todo.add",
        "todo/list",
        vec![FaceInteractionArgument {
            name: "text".into(),
            value_kind: "value/bool".into(),
            value: vec![1],
        }],
    );
    assert_eq!(
        todo_command_from_interaction(&state, &forged, &forged_show, &malformed),
        Err(TodoFaceError::InvalidActionContract)
    );

    let mut actions = forged.actions.clone();
    let add_action = actions
        .iter_mut()
        .find(|item| item.identity == "todo.add")
        .unwrap();
    add_action.arguments = vec![FaceActionArgument::text(
        "text".into(),
        "Item text".into(),
        1,
        conduit_todo_plot::MAX_TODO_TEXT_BYTES as u32,
    )
    .unwrap()];
    add_action.intent = "todo/other@1".into();
    let (wrong_intent, wrong_intent_show) = reface(forged, actions);
    let interaction = interaction(
        &wrong_intent,
        &wrong_intent_show,
        "todo.add",
        "todo/list",
        add.arguments,
    );
    assert_eq!(
        todo_command_from_interaction(&state, &wrong_intent, &wrong_intent_show, &interaction),
        Err(TodoFaceError::InvalidActionContract)
    );
}
