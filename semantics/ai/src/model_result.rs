use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    ConfidencePermille, LlmDeterminismProfile, LlmSemanticContract, LlmTerminalOutcome,
    ModelResultDisposition, ModelResultInvalidity, ModelResultProvenance, ModelWorkAccounting,
};

impl ModelResultDisposition {
    pub const fn terminal_outcome(self) -> LlmTerminalOutcome {
        match self {
            Self::Produced => LlmTerminalOutcome::Produced,
            Self::Truncated => LlmTerminalOutcome::Truncated,
            Self::Refused(_) => LlmTerminalOutcome::Refused,
            Self::Failed(_) => LlmTerminalOutcome::Failed,
            Self::Cancelled => LlmTerminalOutcome::Cancelled,
            Self::ProviderLost => LlmTerminalOutcome::ProviderLost,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Established serde carrier for the authored `ModelDerivedResult<ModelResultPayload>` Type.
pub struct ModelDerivedResult {
    pub provenance: ModelResultProvenance,
    pub payload_kind: String,
    pub payload: Vec<u8>,
    pub implementation_identity: String,
    pub request_identity: String,
    pub run_identity: String,
    pub confidence: Option<ConfidencePermille>,
    pub disposition: ModelResultDisposition,
    pub determinism: LlmDeterminismProfile,
    pub accounting: ModelWorkAccounting,
}

impl ModelDerivedResult {
    pub fn validate(&self, contract: &LlmSemanticContract) -> Result<(), ModelResultInvalidity> {
        if self.implementation_identity.is_empty()
            || self.request_identity.is_empty()
            || self.run_identity.is_empty()
        {
            return Err(ModelResultInvalidity::MissingExactIdentity);
        }
        if self.payload_kind != contract.result_payload_kind.as_str() {
            return Err(ModelResultInvalidity::UnsupportedPayloadKind);
        }
        if self.accounting.input_bytes > contract.bounds.maximum_input_bytes() {
            return Err(ModelResultInvalidity::InputBoundExceeded);
        }
        if self.accounting.context_items > contract.bounds.maximum_context_items() {
            return Err(ModelResultInvalidity::ContextBoundExceeded);
        }
        if self.accounting.output_bytes > contract.bounds.maximum_output_bytes() {
            return Err(ModelResultInvalidity::OutputBoundExceeded);
        }
        if self.accounting.work_units > contract.bounds.maximum_work_units() {
            return Err(ModelResultInvalidity::WorkBoundExceeded);
        }
        if self.accounting.history_items > contract.bounds.maximum_history_items() {
            return Err(ModelResultInvalidity::HistoryBoundExceeded);
        }
        if self.accounting.output_bytes != self.payload.len() as u64 {
            return Err(ModelResultInvalidity::PayloadLengthMismatch);
        }
        match self.disposition {
            ModelResultDisposition::Produced | ModelResultDisposition::Truncated
                if self.payload.is_empty() =>
            {
                Err(ModelResultInvalidity::ProducedPayloadMissing)
            }
            ModelResultDisposition::Refused(_)
            | ModelResultDisposition::Failed(_)
            | ModelResultDisposition::Cancelled
            | ModelResultDisposition::ProviderLost
                if !self.payload.is_empty() =>
            {
                Err(ModelResultInvalidity::TerminalPayloadPresent)
            }
            _ => Ok(()),
        }
    }
}
