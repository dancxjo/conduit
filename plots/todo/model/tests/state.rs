use conduit_todo_plot::{todo_combine_kind, TodoCommand, TodoRefusal, TodoState, MAX_TODO_ITEMS};

#[test]
fn combine_kind_has_exact_typed_state_and_command_ports() {
    let kind = todo_combine_kind();
    kind.validate().unwrap();
    assert_eq!(
        kind.inputs[0].value_kind.as_str(),
        conduit_todo_plot::TODO_STATE_INFO_ID
    );
    assert_eq!(
        kind.inputs[1].value_kind.as_str(),
        conduit_todo_plot::TODO_COMMAND_INFO_ID
    );
    assert_eq!(
        kind.outputs[0].value_kind.as_str(),
        conduit_todo_plot::TODO_STATE_INFO_ID
    );
}

#[test]
fn typed_command_codec_refuses_unknown_shapes_and_preserves_arguments() {
    for command in [
        TodoCommand::Add {
            text: "Buy milk".into(),
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        },
        TodoCommand::Remove {
            id: "task-1".into(),
        },
    ] {
        assert_eq!(
            TodoCommand::decode_info(&command.encode_info().unwrap()).unwrap(),
            command
        );
    }
    let unknown = conduit_web::JsonValue::decode_text(br#"{"op":"clear"}"#)
        .unwrap()
        .encode_info()
        .unwrap();
    assert_eq!(
        TodoCommand::decode_info(&unknown),
        Err(TodoRefusal::InvalidCommand)
    );
    assert_eq!(
        TodoCommand::Add { text: " ".into() }.encode_info(),
        Err(TodoRefusal::InvalidText)
    );
}

#[test]
fn add_complete_reopen_remove_preserves_stable_identity_and_order() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let state = state
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    let state = state
        .apply(&TodoCommand::Add {
            text: "Get cat food".into(),
        })
        .unwrap();
    assert_eq!(
        state
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["task-1", "task-2"]
    );
    let completed = state
        .apply(&TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        })
        .unwrap();
    assert!(completed.items[0].complete);
    assert_eq!(completed.revision, 3);
    assert_eq!(
        completed
            .apply(&TodoCommand::SetComplete {
                id: "task-1".into(),
                complete: true
            })
            .unwrap(),
        completed
    );
    let reopened = completed
        .apply(&TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: false,
        })
        .unwrap();
    assert!(!reopened.items[0].complete);
    let removed = reopened
        .apply(&TodoCommand::Remove {
            id: "task-1".into(),
        })
        .unwrap();
    assert_eq!(removed.items[0].id, "task-2");
    assert_eq!(removed.next_id, 3);
    assert_eq!(
        TodoState::decode_info(&removed.encode_info().unwrap()).unwrap(),
        removed
    );
}

#[test]
fn invalid_targets_and_text_refuse_without_state_change() {
    let state = TodoState::new("Groceries".into()).unwrap();
    assert_eq!(
        state.apply(&TodoCommand::Add { text: "  ".into() }),
        Err(TodoRefusal::InvalidText)
    );
    assert_eq!(
        state.apply(&TodoCommand::Remove {
            id: "task-1".into()
        }),
        Err(TodoRefusal::MissingItem)
    );
    assert_eq!(
        state.apply(&TodoCommand::SetComplete {
            id: "other".into(),
            complete: true
        }),
        Err(TodoRefusal::InvalidId)
    );
    assert_eq!(state.revision, 0);
    assert!(state.items.is_empty());
}

#[test]
fn twenty_items_fit_but_the_next_add_refuses_before_mutation() {
    let mut state = TodoState::new("Groceries".into()).unwrap();
    for index in 0..MAX_TODO_ITEMS {
        state = state
            .apply(&TodoCommand::Add {
                text: format!("Item {index}"),
            })
            .unwrap();
    }
    assert_eq!(state.items.len(), 20);
    assert_eq!(
        state.apply(&TodoCommand::Add {
            text: "One more".into()
        }),
        Err(TodoRefusal::ItemCapacity)
    );
    assert_eq!(
        TodoState::decode_info(&state.encode_info().unwrap()).unwrap(),
        state
    );
}

#[test]
fn finite_json_budget_refuses_an_oversized_collection_without_partial_commit() {
    let mut state = TodoState::new("Groceries".into()).unwrap();
    let text = "x".repeat(72);
    let mut accepted = 0;
    for _ in 0..MAX_TODO_ITEMS {
        match state.apply(&TodoCommand::Add { text: text.clone() }) {
            Ok(next) => {
                state = next;
                accepted += 1;
            }
            Err(TodoRefusal::Json(_) | TodoRefusal::Collection(_)) => break,
            Err(error) => panic!("unexpected refusal: {error:?}"),
        }
    }
    assert!(accepted > 0);
    assert_eq!(state.items.len(), accepted);
    assert_eq!(
        TodoState::decode_info(&state.encode_info().unwrap()).unwrap(),
        state
    );
}
