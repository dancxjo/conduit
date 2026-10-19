//! Focused shared Language consumers; fixtures do not establish model accuracy.
use crate::process::Step;

pub const STEPS: &[Step] = &[Step::new(
    "prosody-commitment.source-plan-play",
    "Check committed dependency correlation, symbolic prosody and revision safety",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-language",
        "--test",
        "prosody_commitment",
        "--test",
        "prosody_revision",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
