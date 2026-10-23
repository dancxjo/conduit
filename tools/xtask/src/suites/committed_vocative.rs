//! Narrow authored vocative consumer through current authority and installed PCM.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "committed-vocative.installed-consumer",
    "Check three supplied committed vocatives, exact carrier custody and actual compact PCM",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-speech",
        "--features",
        "semantic-bindings,kernel",
        "--test",
        "committed_vocative",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entrance_selects_installed_tests_and_one_worker() {
        let args = STEPS[0].args;
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--test", "committed_vocative"]));
        assert!(args.contains(&"semantic-bindings,kernel"));
        assert!(args.contains(&"--test-threads=1"));
        assert!(!args.contains(&"--ignored"));
    }
}
