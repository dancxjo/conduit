//! Optional durable Body ownership for exact purpose truth.
//!
//! Purpose is attached only when a resident application has a declared
//! completion contract. It is not a universal Body personality, and readiness
//! derived from it grants no lifecycle authority.

use crate::{
    derive_fulfillment_readiness, Body, BodyId, BodyState, FulfillmentReadiness, PurposeRefusal,
    PurposeState,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPurpose {
    pub body_id: BodyId,
    pub purpose: PurposeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PurposeContinuityRefusal {
    InvalidBody,
    InvalidSemanticState(PurposeRefusal),
    BodyMismatch,
    PurposeIdentityChanged,
    StalePurposeRevision,
    RevisionOverflow,
    BodyFulfilled,
}

impl BodyPurpose {
    pub fn establish(body: &Body, purpose: PurposeState) -> Result<Self, PurposeContinuityRefusal> {
        body.validate()
            .map_err(|_| PurposeContinuityRefusal::InvalidBody)?;
        purpose
            .validate()
            .map_err(PurposeContinuityRefusal::InvalidSemanticState)?;
        Ok(Self {
            body_id: body.body_id.clone(),
            purpose,
        })
    }

    pub fn validate_for(&self, body: &Body) -> Result<(), PurposeContinuityRefusal> {
        body.validate()
            .map_err(|_| PurposeContinuityRefusal::InvalidBody)?;
        if self.body_id != body.body_id {
            return Err(PurposeContinuityRefusal::BodyMismatch);
        }
        self.purpose
            .validate()
            .map_err(PurposeContinuityRefusal::InvalidSemanticState)
    }

    pub fn revise_purpose(&self, next: PurposeState) -> Result<Self, PurposeContinuityRefusal> {
        next.validate()
            .map_err(PurposeContinuityRefusal::InvalidSemanticState)?;
        if next.purpose_id != self.purpose.purpose_id {
            return Err(PurposeContinuityRefusal::PurposeIdentityChanged);
        }
        let expected = self
            .purpose
            .revision
            .checked_add(1)
            .ok_or(PurposeContinuityRefusal::RevisionOverflow)?;
        if next.revision != expected {
            return Err(PurposeContinuityRefusal::StalePurposeRevision);
        }
        Ok(Self {
            body_id: self.body_id.clone(),
            purpose: next,
        })
    }

    /// Readiness is current application truth only while the Body can act.
    pub fn active_readiness(
        &self,
        body: &Body,
    ) -> Result<FulfillmentReadiness, PurposeContinuityRefusal> {
        self.validate_for(body)?;
        if matches!(body.state, BodyState::Fulfilled { .. }) {
            return Err(PurposeContinuityRefusal::BodyFulfilled);
        }
        self.inspect_readiness()
    }

    /// Final purpose and derived readiness remain inspectable after Fulfillment.
    pub fn inspect_readiness(&self) -> Result<FulfillmentReadiness, PurposeContinuityRefusal> {
        derive_fulfillment_readiness(&self.purpose)
            .map_err(PurposeContinuityRefusal::InvalidSemanticState)
    }
}
