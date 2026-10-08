use conduit_todo_plot::{
    apply_transition_packet, todo_apply_kind, todo_combine_kind, todo_pack_kind,
    PreparedTodoPacket, TodoCommand, TodoRefusal, TodoState, MAX_TODO_ITEMS,
};

#[test]
fn combine_kind_has_exact_typed_state_and_command_ports() {
    for kind in [todo_combine_kind(), todo_pack_kind(), todo_apply_kind()] {
        kind.validate().unwrap();
    }
    let kind = todo_combine_kind();
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
fn preallocated_transition_packet_round_trips_exact_values_and_refuses_malformed() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let command = TodoCommand::Add {
        text: "Buy milk".into(),
    };
    let mut packet = PreparedTodoPacket::new();
    let capacity = packet.allocation_capacity();
    let output = apply_transition_packet(
        packet
            .pack(
                &state.encode_info().unwrap(),
                &command.encode_info().unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        TodoState::decode_info(&output).unwrap().items[0].text,
        "Buy milk"
    );
    assert_eq!(packet.allocation_capacity(), capacity);
    let mut malformed = packet.bytes().to_vec();
    malformed.pop();
    assert_eq!(
        apply_transition_packet(&malformed),
        Err(TodoRefusal::InvalidTransition)
    );
}

#[cfg(feature = "kernel-step")]
#[test]
fn packet_back_stages_two_exact_inputs_without_allocating_during_step() {
    use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
    use conduit_kernel::{PortId, ValueRef};
    use conduit_todo_plot::TodoPacketBack;

    let state = TodoState::new("Groceries".into())
        .unwrap()
        .encode_info()
        .unwrap();
    let command = TodoCommand::Add {
        text: "Buy milk".into(),
    }
    .encode_info()
    .unwrap();
    let mut back = TodoPacketBack::new();
    let capacity = back.allocation_capacity();
    let mut io = StepIo::<2>::test_frame(
        [
            Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: state.len() as u32,
            }),
            Some(ValueRef {
                slot: 1,
                generation: 1,
                byte_len: command.len() as u32,
            }),
        ],
        [false, false],
        [
            Some(conduit_todo_plot::TODO_TRANSITION_MAX_BYTES as u32),
            None,
        ],
        None,
        8,
    );
    let inputs = StepInputBytes::test_frame([Some(&state), Some(&command)], None);
    assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Progress));
    assert!(io.test_consumed(PortId(0)) && io.test_consumed(PortId(1)));
    assert_eq!(io.test_prepared_output().unwrap().0, PortId(0));
    let packet = <TodoPacketBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap();
    assert_eq!(
        TodoState::decode_info(&apply_transition_packet(packet).unwrap())
            .unwrap()
            .items[0]
            .text,
        "Buy milk"
    );
    assert_eq!(back.allocation_capacity(), capacity);
    <TodoPacketBack as StepBack<2>>::step_committed(&mut back);
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
