//! Explicit installed-runtime Source/Plan/Play proof; never fetches artifacts.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
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
        assert_eq!(STEPS.len(), 1);
        let args = STEPS[0].args;
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
