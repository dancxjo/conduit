//! Canonical phones, committed authority, installed Source and compact PCM.
use crate::process::Step;

// One Cargo graph unifies the Speech/StdHost features and avoids recompiling
// their shared dependencies between the two installed proof boundaries.
pub const STEPS: &[Step] = &[Step::new(
    "record-pronunciation.committed-consumer",
    "Check canonical phones, current authority, installed Source Plan/Play and compact PCM",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-speech",
        "-p",
        "conduit-std-host",
        "--features",
        "conduit-speech/semantic-bindings,conduit-speech/kernel",
        "--test",
        "record_pronunciation",
        "--test",
        "record_pronunciation_installed",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
