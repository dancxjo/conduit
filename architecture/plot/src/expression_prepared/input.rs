//! Unforgeable association of borrowed canonical bytes and their checked view.
use super::Refusal;
use conduit_core::ValidatedCanonicalStructuredValue;

#[derive(Clone, Copy)]
pub(super) struct EvaluationInput<'a> {
    bytes: &'a [u8],
    structured: Option<ValidatedCanonicalStructuredValue<'a>>,
}

impl<'a> EvaluationInput<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            structured: None,
        }
    }
    pub(super) fn bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub(super) fn validate_structured(mut self) -> Result<Self, Refusal> {
        if self.structured.is_none() {
            self.structured = Some(
                conduit_core::validate_canonical_structured_value(self.bytes)
                    .map_err(|_| Refusal::InvalidInput)?,
            );
        }
        Ok(self)
    }
    pub(super) fn structured(self) -> Result<ValidatedCanonicalStructuredValue<'a>, Refusal> {
        self.validate_structured()?
            .structured
            .ok_or(Refusal::InvalidInput)
    }
}

/// Created only after validating the complete canonical value; it cannot be
/// paired with different bytes, and does not itself assert a program's Type.
pub(crate) struct PreparedCanonicalInput<'a>(EvaluationInput<'a>);
impl<'a> PreparedCanonicalInput<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, Refusal> {
        Ok(Self(EvaluationInput::new(bytes).validate_structured()?))
    }
    pub(super) fn input(&self) -> EvaluationInput<'a> {
        self.0
    }
}
