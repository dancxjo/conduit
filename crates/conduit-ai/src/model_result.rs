use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{LlmDeterminismProfile, LlmSemanticContract, LlmTerminalOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelResultProvenance {
    ModelDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidencePermille(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRefusal {
    UnsupportedRequest,
    PolicyDenied,
    ContextUnavailable,
    CapacityUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFailure {
    MalformedResult,
    ImplementationFailure,
    ResourceExhausted,
    OutputBoundExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelResultDisposition {
    Produced,
    Truncated,
    Refused(ModelRefusal),
    Failed(ModelFailure),
    Cancelled,
    ProviderLost,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelWorkAccounting {
    pub input_bytes: u64,
    pub context_items: u64,
    pub output_bytes: u64,
    pub work_units: u64,
    pub history_items: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelResultInvalidity {
    MissingExactIdentity,
    UnsupportedPayloadKind,
    InvalidConfidence,
    InputBoundExceeded,
    ContextBoundExceeded,
    OutputBoundExceeded,
    WorkBoundExceeded,
    HistoryBoundExceeded,
    PayloadLengthMismatch,
    TerminalPayloadPresent,
    ProducedPayloadMissing,
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
        if self
            .confidence
            .is_some_and(|confidence| confidence.0 > 1_000)
        {
            return Err(ModelResultInvalidity::InvalidConfidence);
        }
        if self.accounting.input_bytes > contract.bounds.maximum_input_bytes {
            return Err(ModelResultInvalidity::InputBoundExceeded);
        }
        if self.accounting.context_items > contract.bounds.maximum_context_items {
            return Err(ModelResultInvalidity::ContextBoundExceeded);
        }
        if self.accounting.output_bytes > contract.bounds.maximum_output_bytes {
            return Err(ModelResultInvalidity::OutputBoundExceeded);
        }
        if self.accounting.work_units > contract.bounds.maximum_work_units {
            return Err(ModelResultInvalidity::WorkBoundExceeded);
        }
        if self.accounting.history_items > contract.bounds.maximum_history_items {
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
