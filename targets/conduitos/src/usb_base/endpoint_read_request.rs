//! Exact request decoding; the selected native owner supplies the storage bound.
use super::endpoint_read_contract::{ENDPOINT_READ_DATA_BYTES, EndpointReadContract};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointReadRequestRefusal {
    Canonical(StructuredInfoRefusal),
    InvalidEnvelope,
    Length,
}

pub struct PreparedEndpointReadRequestDecoder {
    validator: PreparedStructuredValueValidator,
}
impl PreparedEndpointReadRequestDecoder {
    pub fn new(contract: &EndpointReadContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode(&self, input: &[u8], maximum: u16) -> Result<u16, EndpointReadRequestRefusal> {
        use EndpointReadRequestRefusal as Error;
        if maximum == 0 || maximum > ENDPOINT_READ_DATA_BYTES {
            return Err(Error::InvalidEnvelope);
        }
        self.validator.validate(input).map_err(Error::Canonical)?;
        let value = validate_canonical_structured_value(input).map_err(Error::Canonical)?;
        let length = value
            .record_field("length")
            .map_err(Error::Canonical)?
            .ok_or(Error::Canonical(StructuredInfoRefusal::WrongRecordFields))?
            .primitive_bytes("value/u64")
            .map_err(Error::Canonical)?;
        let length = u64::from_le_bytes(
            length
                .try_into()
                .map_err(|_| Error::Canonical(StructuredInfoRefusal::WrongType))?,
        );
        if length == 0 || length > u64::from(maximum) {
            return Err(Error::Length);
        }
        Ok(length as u16)
    }
}
