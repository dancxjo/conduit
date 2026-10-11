//! Canonical bounded wire bytes; empty payload explicitly requests an actual ZLP.
use super::endpoint_write_contract::{ENDPOINT_WRITE_DATA_BYTES, EndpointWriteContract};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointWriteRequestRefusal {
    Canonical(StructuredInfoRefusal),
    InvalidEnvelope,
    Length,
}
pub struct PreparedEndpointWriteRequestDecoder {
    validator: PreparedStructuredValueValidator,
}
impl PreparedEndpointWriteRequestDecoder {
    pub fn new(contract: &EndpointWriteContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode<'a>(
        &self,
        input: &'a [u8],
        maximum: u16,
    ) -> Result<&'a [u8], EndpointWriteRequestRefusal> {
        use EndpointWriteRequestRefusal as Error;
        if maximum == 0 || maximum > ENDPOINT_WRITE_DATA_BYTES {
            return Err(Error::InvalidEnvelope);
        }
        self.validator.validate(input).map_err(Error::Canonical)?;
        let value = validate_canonical_structured_value(input).map_err(Error::Canonical)?;
        let wire = value
            .record_field("wire")
            .map_err(Error::Canonical)?
            .ok_or(Error::Canonical(StructuredInfoRefusal::WrongRecordFields))?
            .primitive_bytes("value/bytes")
            .map_err(Error::Canonical)?;
        if wire.len() > usize::from(maximum) {
            return Err(Error::Length);
        }
        // Zero length is deliberately valid; native publication must still enqueue
        // its selected endpoint's zero-length Normal TRB and retain the lease.
        Ok(wire)
    }
}
#[cfg(test)]
mod tests;
