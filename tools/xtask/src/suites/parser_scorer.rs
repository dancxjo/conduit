//! Finite action scoring; linguistic transition integration remains separate.
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
