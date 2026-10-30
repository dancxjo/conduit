//! A passing counterexample establishes the missing I1 behavior, not acceptance.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "action-prompt-i1.missing-fallback",
    "Reproduce reachable keyboard without a current source-authorized alternate prompt",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--lib",
        "action_prompt_i1_",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
