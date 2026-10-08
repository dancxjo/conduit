use conduit_todo_plot::{todo_combine_kind, TodoCommand, TodoRefusal, TodoState, MAX_TODO_ITEMS};

#[cfg(feature = "kernel-step")]
mod allocation_probe {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    pub struct CountingAllocator;
    thread_local! { static COUNTING: Cell<bool> = const { Cell::new(false) }; static COUNT: Cell<usize> = const { Cell::new(0) }; }
    fn count() {
        let _ = COUNTING.try_with(|flag| {
            if flag.get() {
                let _ = COUNT.try_with(|count| count.set(count.get() + 1));
            }
        });
    }
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                count();
            }
            ptr
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                count();
            }
            ptr
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let ptr = unsafe { System.realloc(ptr, layout, size) };
            if !ptr.is_null() {
                count();
            }
            ptr
        }
    }
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;
    pub fn count_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
        COUNT.with(|count| count.set(0));
        COUNTING.with(|flag| flag.set(true));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                COUNTING.with(|flag| flag.set(false));
            }
        }
        let reset = Reset;
        let result = f();
        drop(reset);
        (result, COUNT.with(Cell::get))
    }
}

#[test]
fn exact_typed_combine_kind() {
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
fn semantic_actions_and_exact_form_round_trip() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let state = state
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    assert_eq!(state.items[0].id, "task-1");
    assert_eq!(state.revision, 1);
    let state = TodoState::decode_info(&state.encode_info().unwrap()).unwrap();
    let state = state
        .apply(&TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        })
        .unwrap();
    assert!(state.items[0].complete);
    let state = state
        .apply(&TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: false,
        })
        .unwrap();
    let state = state
        .apply(&TodoCommand::Remove {
            id: "task-1".into(),
        })
        .unwrap();
    assert!(state.items.is_empty());
    assert_eq!(state.revision, 4);
    assert_eq!(state.next_id, 2);
    assert_eq!(
        state.apply(&TodoCommand::Remove {
            id: "task-1".into()
        }),
        Err(TodoRefusal::MissingItem)
    );
}

#[test]
fn finite_capacity_and_malformed_forms_refused() {
    let mut state = TodoState::new("List".into()).unwrap();
    for index in 0..MAX_TODO_ITEMS {
        state = state
            .apply(&TodoCommand::Add {
                text: format!("Task {index}"),
            })
            .unwrap();
    }
    assert_eq!(
        state.apply(&TodoCommand::Add {
            text: "overflow".into()
        }),
        Err(TodoRefusal::ItemCapacity)
    );
    let mut bytes = state.encode_info().unwrap();
    bytes.push(0);
    assert_eq!(
        TodoState::decode_info(&bytes),
        Err(TodoRefusal::InvalidState)
    );
    let command = TodoCommand::Add {
        text: "buy milk".into(),
    };
    assert_eq!(
        TodoCommand::decode_info(&command.encode_info().unwrap()),
        Ok(command)
    );
    assert!(TodoCommand::decode_info(&[1, 2]).is_err());
}

#[cfg(feature = "kernel-step")]
#[test]
fn combine_back_consumes_two_exact_values_with_fixed_storage() {
    use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
    use conduit_kernel::{PortId, ValueRef};
    use conduit_todo_plot::{TodoCombineBack, STATE_MAX_BYTES};

    let state = TodoState::new("List".into())
        .unwrap()
        .encode_info()
        .unwrap();
    let command = TodoCommand::Add {
        text: "Buy milk".into(),
    }
    .encode_info()
    .unwrap();
    let mut back = TodoCombineBack::new();
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
        [Some(STATE_MAX_BYTES as u32), None],
        None,
        8,
    );
    let inputs = StepInputBytes::test_frame([Some(&state), Some(&command)], None);
    let (outcome, allocations) = allocation_probe::count_during(|| back.step(&mut io, &inputs));
    assert!(matches!(outcome, StepOutcome::Progress));
    assert_eq!(allocations, 0);
    assert!(io.test_consumed(PortId(0)) && io.test_consumed(PortId(1)));
    let output = <TodoCombineBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap();
    assert_eq!(
        TodoState::decode_info(output).unwrap().items[0].text,
        "Buy milk"
    );
    assert_eq!(back.allocation_capacity(), 0);
}

#[cfg(feature = "kernel-step")]
#[test]
fn public_actions_and_back_agree_across_sequential_revisions() {
    use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
    use conduit_kernel::{PortId, ValueRef};
    use conduit_todo_plot::{TodoCombineBack, STATE_MAX_BYTES};

    let mut state = TodoState::new("List".into()).unwrap();
    for command in [
        TodoCommand::Add {
            text: "First".into(),
        },
        TodoCommand::Add {
            text: "Second".into(),
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: false,
        },
        TodoCommand::Remove {
            id: "task-2".into(),
        },
    ] {
        let expected = state.apply(&command).unwrap();
        let state_bytes = state.encode_info().unwrap();
        let command_bytes = command.encode_info().unwrap();
        let mut back = TodoCombineBack::new();
        let mut io = StepIo::<2>::test_frame(
            [
                Some(ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: state_bytes.len() as u32,
                }),
                Some(ValueRef {
                    slot: 1,
                    generation: 1,
                    byte_len: command_bytes.len() as u32,
                }),
            ],
            [false, false],
            [Some(STATE_MAX_BYTES as u32), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([Some(&state_bytes), Some(&command_bytes)], None);
        let (outcome, allocations) = allocation_probe::count_during(|| back.step(&mut io, &inputs));
        assert!(matches!(outcome, StepOutcome::Progress));
        assert_eq!(allocations, 0);
        let actual = TodoState::decode_info(
            <TodoCombineBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
        state = actual;
    }
    assert_eq!(state.revision, 5);
    assert_eq!(state.next_id, 3);
    assert_eq!(state.items.len(), 1);
}
