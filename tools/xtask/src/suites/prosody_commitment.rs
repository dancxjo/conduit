//! Focused shared Language consumers; fixtures do not establish model accuracy.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new(
        "prosody-commitment.revision-context",
        "Check generic observation context, dependency windows and correction protection",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-core",
            "-p",
            "conduit-language",
            "--test",
            "revision",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
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
    ),
];
