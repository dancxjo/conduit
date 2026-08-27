//! Model-derived follow-up timing remains a proposal until ordinary admission.

use alloc::string::String;
use conduit_core::MAXIMUM_TEMPORAL_IDENTITY_BYTES;
use conduit_time::{ScheduledIntent, ScheduledIntentRefusal};
use serde::{Deserialize, Serialize};

use crate::{ModelEffectProposal, ModelResultProvenance};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFollowUpTimingProposal {
    pub identity: String,
    pub provenance: ModelResultProvenance,
    pub proposed: ScheduledIntent<ModelEffectProposal>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelFollowUpRefusal {
    InvalidIdentity,
    InvalidSchedule(ScheduledIntentRefusal),
}

impl ModelFollowUpTimingProposal {
    pub fn validate(&self) -> Result<(), ModelFollowUpRefusal> {
        if self.identity.is_empty() || self.identity.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
            return Err(ModelFollowUpRefusal::InvalidIdentity);
        }
        self.proposed
            .validate()
            .map_err(ModelFollowUpRefusal::InvalidSchedule)
    }

    /// Returns the proposal payload without creating an authorized request.
    /// `ProposalGate` remains the only transition into an effect request.
    pub const fn effect_proposal(&self) -> &ModelEffectProposal {
        &self.proposed.payload
    }
}
