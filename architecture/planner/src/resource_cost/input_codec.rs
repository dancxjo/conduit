//! Domain-owned codec admission from the trusted semantic catalogue. Requests
//! supply values, never codec implementations or workload-class labels.
use super::{CostEvidenceRefusal as Refusal, MAXIMUM_COST_ID_BYTES, MAXIMUM_COST_INPUT_BYTES};
use conduit_core::{ArtifactId, KindId, ValuePayload};

/// Reviewed owner decoder invoked only during bounded preparation. The owner
/// retains canonical spelling, semantic field checks and codec work limits;
/// the generic Planner additionally enforces the admitted input byte extent.
pub struct CostInputCodec<'a> {
    kind: &'a KindId,
    revision: &'a ArtifactId,
    maximum_bytes: usize,
    validate: fn(&[u8]) -> Result<(), Refusal>,
}
impl<'a> CostInputCodec<'a> {
    pub fn admit(
        kind: &'a KindId,
        revision: &'a ArtifactId,
        maximum_bytes: usize,
        validate: fn(&[u8]) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        if kind.as_str().is_empty()
            || kind.as_str().len() > MAXIMUM_COST_ID_BYTES
            || revision.as_str().is_empty()
            || revision.as_str().len() > MAXIMUM_COST_ID_BYTES
        {
            return Err(Refusal::InvalidIdentity);
        }
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_COST_INPUT_BYTES {
            return Err(Refusal::InputLimit);
        }
        if super::structured_input::known_leaf(kind.as_str()) {
            return Err(Refusal::InvalidWorkInput);
        }
        Ok(Self {
            kind,
            revision,
            maximum_bytes,
            validate,
        })
    }
    pub fn kind(&self) -> &'a KindId {
        self.kind
    }
    pub fn revision(&self) -> &'a ArtifactId {
        self.revision
    }
    pub fn validate(&self, value: &ValuePayload) -> Result<(), Refusal> {
        if &value.value_kind != self.kind {
            return Err(Refusal::InvalidWorkInput);
        }
        if value.encoded.len() > self.maximum_bytes {
            return Err(Refusal::InputLimit);
        }
        (self.validate)(&value.encoded)
    }
}
