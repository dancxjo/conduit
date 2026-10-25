//! Actual Linux immutable file effects, separate from Host/Body admission.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "linux-snapshot.records",
        "Check the shared immutable record codec",
        "cargo",
        &["test", "--locked", "-p", "conduit-resource-snapshot"],
    ),
    Step::new(
        "linux-snapshot.files",
        "Check actual immutable file publication and recovery",
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
        "Lint Linux file-effect custody",
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
