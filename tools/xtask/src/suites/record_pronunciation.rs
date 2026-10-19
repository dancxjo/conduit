//! Supplied committed syntax, canonical phones and installed compact synthesis.
use crate::process::Step;

pub const STEPS: &[Step] = &[Step::new(
    "record-pronunciation.source-plan-play",
    "Check syntax-sensitive record phones, authority refusals and installed PCM",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-speech",
        "--features",
        "semantic-bindings,kernel",
        "--test",
        "record_pronunciation",
        "--test",
        "record_pronunciation_installed",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
