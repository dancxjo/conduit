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
), Step::new(
    "record-pronunciation.std-source-plan-play",
    "Execute exact pronunciation Source through installed StdHost and compare complete receipts",
    "cargo", &[
        "test", "--locked", "-p", "conduit-std-host", "--test", "record_pronunciation",
        "--", "--nocapture", "--test-threads=1",
    ],
)];
