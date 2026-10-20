//! Generic finite numerical custody; no parser policy, training or model download.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "categorical-runtime.custody",
    "Check bounded invocation custody, actual kernel ACKs, refusals and retained Sign capacity",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-ai",
        "--features",
        "kernel-operation-owners",
        "--test",
        "integer_categorical_step",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
