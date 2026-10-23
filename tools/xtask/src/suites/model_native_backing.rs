//! Explicit installed-runtime proof; never obtains model/runtime artifacts.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "model.native-backing-unit",
        "Compile optional native profile and verify finite backing contracts",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--lib",
            "native_model_backing::",
            "--",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-backing-live",
        "Admit the caller-configured installed CPU runtime and prove fixed backing/pressure",
        "cargo",
        &[
            "run",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--bin",
            "conduit-native-model-worker",
            "--",
            "--prove",
        ],
    ),
];
