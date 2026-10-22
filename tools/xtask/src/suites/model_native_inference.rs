//! Explicit installed-runtime Source/Plan/Play proof; never fetches artifacts.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "model.native-inference-unit",
    "Verify native scalar profile eligibility without a runtime download",
    "cargo",
    &["test", "--locked", "-p", "conduit-std-host", "--features", "native-model-backing", "--lib", "hosted_native_model::"],
), Step::new(
    "model.native-inference",
    "Run actual admitted native CPU inference through Source, Plan and Play with parent allocation controls",
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
)];
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
