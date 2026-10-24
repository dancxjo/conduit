//! Owned timer contract proof; providers and time are explicit CPU fixtures.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "monotonic-wait.contract",
        "Check finite owned waits, retained configuration, cancellation and retirement",
        "cargo",
        &[
            "test",
            "--locked",
            "--manifest-path",
            "mechanisms/io/monotonic-wait/Cargo.toml",
        ],
    ),
    Step::new(
        "monotonic-wait.lint",
        "Lint the native wait boundary and its contract fixtures",
        "cargo",
        &[
            "clippy",
            "--locked",
            "--manifest-path",
            "mechanisms/io/monotonic-wait/Cargo.toml",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
];
