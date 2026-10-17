//! Finite parser artifact and checked optional action selection.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "parser-scorer.artifact",
    "Admit exact learned weights and measure held-out action scoring with generic numerical execution",
    "cargo",
    &["test", "--locked", "-p", "conduit-language", "--test", "parser_scorer_artifact", "--", "--nocapture"],
)];
pub const REPRODUCE: &[Step] = &[Step::new(
    "parser-scorer.reproduce",
    "Reproduce pinned corpus partitions, exact learned weights and metrics without updating the artifact",
    "python3",
    &["semantics/language/training/reproduce.py"],
)];

pub const SELECTION: &[Step] = &[Step::new(
    "parser-selection.plan-play",
    "Exercise optional admitted scoring, checked legal transitions and incremental Source revisions",
    "cargo",
    &["test", "--locked", "-p", "conduit-language", "--features", "parser-model", "--test", "parser_scorer_artifact", "--test", "parser_model_admission", "--test", "parser_selection", "--test", "lexical", "--", "--nocapture", "--test-threads=1"],
)];
