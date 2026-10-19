//! One existing checked rebase expression and actual public execution custody.
use crate::process::Step;

pub const STEPS: &[Step] = &[Step::new(
    "parser-rebase-replay.source-correlation",
    "Check reusable rebase replay, forged outputs and actual Source Plan/Play",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-language",
        "--test",
        "parser_rebase_replay",
        "--test",
        "parser_revision_protection",
        "--",
        "--nocapture",
        "--test-threads=1",
    ],
)];
