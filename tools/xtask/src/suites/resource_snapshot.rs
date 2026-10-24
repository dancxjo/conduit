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
        "resource-snapshot.browser-operations",
        "Check browser snapshot request and storage acknowledgements",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-browser-runtime",
            "--no-default-features",
            "--features",
            "plot-runner",
            "--lib",
            "plot_runner::resource_effect_tests",
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
