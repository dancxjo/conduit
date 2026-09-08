//! Semantic lifetime policy sealed by a checked Form into its exact Plan.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlanCompletionPolicy {
    #[default]
    Live,
    SemanticCompletion,
}
