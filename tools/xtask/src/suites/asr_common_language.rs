//! Focused shared Language consumers; fixtures do not establish model accuracy.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new(
        "asr-common-language.source-plan-play",
        "Feed classified ASR revisions through the shared Language spine",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-speech",
            "--features",
            "semantic-bindings",
            "--test",
            "asr_common_language",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "asr-common-language.installed-std",
        "Execute the same classified revisions through installed StdHost",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "asr_common_language",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
];
