//! Durable Body ownership for character and purpose semantic truth.

use crate::{
    derive_character_context, Body, BodyId, BodyState, CharacterContext, CharacterProfile,
    CharacterPurposeRefusal, PurposeState,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyCharacterPurpose {
    pub body_id: BodyId,
    pub character: CharacterProfile,
    pub purpose: PurposeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterPurposeContinuityRefusal {
    InvalidBody,
    InvalidSemanticState(CharacterPurposeRefusal),
    BodyMismatch,
    PurposeIdentityChanged,
    StalePurposeRevision,
    RevisionOverflow,
    BodyFulfilled,
}

impl BodyCharacterPurpose {
    pub fn establish(
        body: &Body,
        character: CharacterProfile,
        purpose: PurposeState,
    ) -> Result<Self, CharacterPurposeContinuityRefusal> {
        body.validate()
            .map_err(|_| CharacterPurposeContinuityRefusal::InvalidBody)?;
        character
            .validate()
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)?;
        purpose
            .validate()
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)?;
        Ok(Self {
            body_id: body.body_id.clone(),
            character,
            purpose,
        })
    }

    pub fn validate_for(&self, body: &Body) -> Result<(), CharacterPurposeContinuityRefusal> {
        body.validate()
            .map_err(|_| CharacterPurposeContinuityRefusal::InvalidBody)?;
        if self.body_id != body.body_id {
            return Err(CharacterPurposeContinuityRefusal::BodyMismatch);
        }
        self.character
            .validate()
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)?;
        self.purpose
            .validate()
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)
    }

    pub fn revise_purpose(
        &self,
        next: PurposeState,
    ) -> Result<Self, CharacterPurposeContinuityRefusal> {
        next.validate()
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)?;
        if next.purpose_id != self.purpose.purpose_id {
            return Err(CharacterPurposeContinuityRefusal::PurposeIdentityChanged);
        }
        let expected = self
            .purpose
            .revision
            .checked_add(1)
            .ok_or(CharacterPurposeContinuityRefusal::RevisionOverflow)?;
        if next.revision != expected {
            return Err(CharacterPurposeContinuityRefusal::StalePurposeRevision);
        }
        Ok(Self {
            body_id: self.body_id.clone(),
            character: self.character.clone(),
            purpose: next,
        })
    }

    /// Current semantic context exists only while the Body can still act.
    pub fn active_context(
        &self,
        body: &Body,
    ) -> Result<CharacterContext, CharacterPurposeContinuityRefusal> {
        self.validate_for(body)?;
        if matches!(body.state, BodyState::Fulfilled { .. }) {
            return Err(CharacterPurposeContinuityRefusal::BodyFulfilled);
        }
        self.inspect_context()
    }

    /// Final character and purpose remain inspectable after Fulfillment.
    pub fn inspect_context(&self) -> Result<CharacterContext, CharacterPurposeContinuityRefusal> {
        derive_character_context(&self.character, &self.purpose)
            .map_err(CharacterPurposeContinuityRefusal::InvalidSemanticState)
    }
}
