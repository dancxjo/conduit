//! Prepared exact-schema decoding borrows admitted request storage during Play.
use super::{
    contract::SpiContract,
    transaction::{SpiDisposition, SpiTransaction},
};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

pub struct PreparedSpiDecoder {
    validator: PreparedStructuredValueValidator,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpiDecodeRefusal {
    Canonical(StructuredInfoRefusal),
    Geometry(SpiDisposition),
}

impl PreparedSpiDecoder {
    pub fn new(contract: &SpiContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode<'a>(&self, input: &'a [u8]) -> Result<SpiTransaction<'a>, SpiDecodeRefusal> {
        self.canonical(input)
            .map_err(SpiDecodeRefusal::Canonical)
            .and_then(|bytes| SpiTransaction::new(bytes).map_err(SpiDecodeRefusal::Geometry))
    }
    fn canonical<'a>(&self, input: &'a [u8]) -> Result<&'a [u8], StructuredInfoRefusal> {
        self.validator.validate(input)?;
        let value = validate_canonical_structured_value(input)?;
        value
            .record_field("output")?
            .ok_or(StructuredInfoRefusal::WrongRecordFields)?
            .primitive_bytes("value/bytes")
    }
}
