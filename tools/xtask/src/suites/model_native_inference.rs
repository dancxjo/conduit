//! Explicit installed-runtime Source/Plan/Play proof; never fetches artifacts.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "model.native-inference-unit",
        "Verify native scalar profile eligibility without a runtime download",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--lib",
            "hosted_native_model::",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-transport",
        "Verify bounded native framing and preparation cleanup",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--lib",
            "native_model_backing::supervisor::",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-owner",
        "Verify exact pending Host Call cancellation custody",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--lib",
            "installed_std::model_work_pending::",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-pending",
        "Verify retained input and terminal output custody",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--test",
            "model_infer_pending",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-pending-play",
        "Verify ordinary Value Play cancellation and warm reuse",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--test",
            "model_infer_pending_play",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-inference",
        "Run actual admitted native CPU inference through Source Plan Play with parent allocation controls",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--test",
            "model_infer_native",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "model.native-lifecycle",
        "Run actual native warm reuse pending cancellation and provider loss",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--features",
            "native-model-backing",
            "--test",
            "model_infer_native_lifecycle",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_native_proof_executes_platform_tests_instead_of_silently_ignoring_them() {
        let args = STEPS
            .iter()
            .find(|step| step.id == "model.native-inference")
            .unwrap()
            .args;
        assert!(
            args.contains(&"--ignored"),
            "actual-runtime tests require explicit selection"
        );
        assert!(
            args.contains(&"--test-threads=1"),
            "parent allocation proof has one owner"
        );
        assert!(args
            .windows(2)
            .any(|v| v == ["--test", "model_infer_native"]));
    }
}
