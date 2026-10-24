//! Actual local Linux file effects, without a Body, store offer or scheduler.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "linux-snapshot.files",
        "Check bounded immutable file publication and restoration",
        "cargo",
        &[
            "test",
            "--locked",
            "--manifest-path",
            "targets/std/resource-snapshot/Cargo.toml",
        ],
    ),
    Step::new(
        "linux-snapshot.lint",
        "Lint the Linux file boundary and its filesystem proofs",
        "cargo",
        &[
            "clippy",
            "--locked",
            "--manifest-path",
            "targets/std/resource-snapshot/Cargo.toml",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
];
