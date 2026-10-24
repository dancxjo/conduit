//! Shared immutable record contract and its existing browser public path.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "resource-snapshot.records",
        "Check immutable snapshot records",
        "cargo",
        &["test", "--locked", "-p", "conduit-resource-snapshot"],
    ),
    Step::new(
        "resource-snapshot.browser",
        "Check browser record compatibility",
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
        "Lint shared snapshot record custody",
        "cargo",
        &[
            "clippy",
            "--locked",
            "-p",
            "conduit-resource-snapshot",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
];
