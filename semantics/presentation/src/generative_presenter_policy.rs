//! Finite implementation-owned policy and work bounds for generative presentation.

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAX_GENERATIVE_PRESENTER_POLICY_BYTES: usize = 4_096;
/// The portable `llm/present` capability's reviewed semantic input ceiling.
pub const MAX_GENERATIVE_PRESENTER_INPUT_BYTES: usize = 262_144;
pub const MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES: usize = 16_384;
pub const MAX_GENERATIVE_PRESENTER_TIMEOUT_MILLIS: u32 = 300_000;
pub const MAX_GENERATIVE_PRESENTER_CONCURRENCY: u16 = 16;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerativePresenterBounds {
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
    pub maximum_history_items: u16,
    pub maximum_concurrency: u16,
    pub timeout_millis: u32,
}

impl GenerativePresenterBounds {
    pub const fn reviewed_default() -> Self {
        Self {
            maximum_input_bytes: MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
            maximum_output_bytes: 4_096,
            maximum_history_items: 0,
            maximum_concurrency: 1,
            timeout_millis: 30_000,
        }
    }

    pub(crate) fn valid(&self) -> bool {
        self.maximum_input_bytes > 0
            && self.maximum_input_bytes as usize <= MAX_GENERATIVE_PRESENTER_INPUT_BYTES
            && self.maximum_output_bytes > 0
            && self.maximum_output_bytes as usize <= MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES
            && self.maximum_history_items <= 1
            && self.maximum_concurrency > 0
            && self.maximum_concurrency <= MAX_GENERATIVE_PRESENTER_CONCURRENCY
            && self.timeout_millis > 0
            && self.timeout_millis <= MAX_GENERATIVE_PRESENTER_TIMEOUT_MILLIS
    }
}

/// Implementation-owned instructions, deliberately separate from semantic data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerativePresenterPolicy {
    pub template_contract_revision: String,
    /// A replaceable presenter speaks as the supplied Body without acquiring
    /// that body's identity, continuity, authority, or stake.
    pub narrator_role: GenerativeNarratorRole,
    pub instructions: String,
}

/// The narrator's implementation role and the voice it performs are distinct.
///
/// Additional voice modes require an explicit reviewed contract revision; the
/// first generative Presenter only admits the body's first-person voice.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GenerativeNarratorRole {
    TransientFirstPersonBodyNarrator,
}
