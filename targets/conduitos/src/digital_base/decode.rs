//! Exact logical requests contain no physical contact selector or direction change.
use super::{DigitalRequest, contract::DigitalContract};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};
pub struct PreparedDigitalDecoder {
    validator: PreparedStructuredValueValidator,
}
#[derive(Debug, PartialEq, Eq)]
pub enum DigitalDecodeRefusal {
    Canonical(StructuredInfoRefusal),
}
impl PreparedDigitalDecoder {
    pub fn new(contract: &DigitalContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode(&self, input: &[u8]) -> Result<DigitalRequest, DigitalDecodeRefusal> {
        self.canonical(input)
            .map_err(DigitalDecodeRefusal::Canonical)
    }
    fn canonical(&self, input: &[u8]) -> Result<DigitalRequest, StructuredInfoRefusal> {
        self.validator.validate(input)?;
        let value = validate_canonical_structured_value(input)?;
        match value.variant_tag()? {
            "observe" => Ok(DigitalRequest::Observe),
            "drive" => {
                let payload = value
                    .variant_payload("drive")?
                    .ok_or(StructuredInfoRefusal::UnknownVariantTag)?;
                let high = payload
                    .record_field("high")?
                    .ok_or(StructuredInfoRefusal::WrongRecordFields)?
                    .primitive_bytes("value/bool")?;
                match high {
                    [0] => Ok(DigitalRequest::Drive(false)),
                    [1] => Ok(DigitalRequest::Drive(true)),
                    _ => Err(StructuredInfoRefusal::WrongType),
                }
            }
            _ => Err(StructuredInfoRefusal::UnknownVariantTag),
        }
    }
}
