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
        "Execute add, toggle and remove through composed Todo Plots and the production kernel",
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
    Step::new(
        "todo-state.browser-kernel",
        "Execute the same Todo Plots through the browser Host production kernel",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-browser-runtime",
            "--lib",
            "browser_todo",
            "--locked",
        ],
    ),
];

pub(super) const TODO_DURABILITY_STEPS: &[Step] = &[
    Step::new(
        "todo-durability.resource",
        "Prove bounded selected resource commits, corruption refusals and failed publication truth",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "todo_durable_resource",
        ],
    ),
    Step::new(
        "todo-durability.write",
        "Prove admitted checkpoint writes and cancellation/resource exhaustion",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "todo_checkpoint_body_play",
        ],
    ),
    Step::new(
        "todo-durability.read",
        "Prove exact selected reads and namespace, schema, generation and authority refusals",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "todo_checkpoint_read_body_play",
        ],
    ),
    Step::new(
        "todo-durability.owner",
        "Prove retained Signs, fresh reads, authenticated second Host and failed-read retry",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            "checkpoint_once::tests",
            "--",
            "--test-threads=1",
        ],
    ),
];
