//! Shared record codec and unchanged browser operation contract; no storage I/O.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "resource-snapshot.records",
        "Check the bounded immutable record codec",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-resource-snapshot",
            "--features",
            "host-call,serde",
        ],
    ),
    Step::new(
        "resource-snapshot.browser-operation",
        "Check browser request correlation against the shared codec",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-browser-runtime",
            "--no-default-features",
            "--lib",
            "resource_snapshot",
        ],
    ),
    Step::new(
        "resource-snapshot.lint",
        "Lint the shared record boundary",
        "cargo",
        &[
            "clippy",
            "--locked",
            "-p",
            "conduit-resource-snapshot",
            "--features",
            "host-call,serde",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
];
