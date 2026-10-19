//! Explicit candidate evaluation; accepted parser defaults remain unchanged.
use crate::process::Step;

pub const GREETING: &[Step] = &[Step::new(
    "parser-punctuation.greeting",
    "Check actual Source pre-final vocative commitment in both revision tracks",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-language",
        "--features",
        "parser-model",
        "--test",
        "parser_punctuation_selection",
        "punctuation_v3_motivating_greeting_actual_source_two_tracks",
        "--",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ],
)];

pub const RETAINED: &[Step] = &[Step::new(
    "parser-punctuation.retained",
    "Evaluate actual Source retained and reserved cases without repeating the greeting",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-language",
        "--features",
        "parser-model",
        "--test",
        "parser_punctuation_selection",
        "punctuation_v3_retained_dev_test_and_other_reserved_actual_source",
        "--",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ],
)];
