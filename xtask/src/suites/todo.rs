use crate::process::Step;

pub(super) const TODO_STATE_STEPS: &[Step] = &[
    Step::new(
        "todo-state.contract",
        "Prove bounded collection edits, exact commands, order and refusals",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-web",
            "--test",
            "json_collection",
            "--locked",
        ],
    ),
    Step::new(
        "todo-state.summary",
        "Prove bounded Boolean-field counts and exact malformed-record refusals",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-web",
            "--test",
            "json_boolean_summary",
            "--locked",
        ],
    ),
    Step::new(
        "todo-state.kernel",
        "Execute add, toggle and remove through composed Todo Forms and the production kernel",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--lib",
            "todo_",
            "--locked",
        ],
    ),
];
