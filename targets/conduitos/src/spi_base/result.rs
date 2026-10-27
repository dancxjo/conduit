//! Prepared, bounded SPI outcomes. No partial result can masquerade as completion.
use super::{
    contract::{SPI_MAXIMUM_BYTES, SpiContract},
    transaction::{MAXIMUM_TRANSACTION_BYTES, SpiDisposition},
};
use alloc::vec::Vec;
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    kind_id, validate_canonical_structured_value,
};

pub struct PreparedSpiResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    input: PreparedStructuredComposer,
    unit: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpiResultRefusal {
    Canonical(StructuredInfoRefusal),
    ActualLength,
}

impl PreparedSpiResultEncoder {
    pub fn new(contract: &SpiContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let completed = cases
            .iter()
            .find(|case| case.tag() == "completed")
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?
            .payload_type();
        let StructuredInfoTypeShape::Record { fields, .. } = completed.shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let input = fields
            .iter()
            .find(|field| field.name() == "input")
            .ok_or(StructuredInfoRefusal::WrongRecordFields)?
            .value_type();
        let mut unit = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(kind_id("value/empty"))?,
            64,
        )?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                SPI_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(completed, SPI_MAXIMUM_BYTES as usize)?,
            input: PreparedStructuredComposer::new(input, SPI_MAXIMUM_BYTES as usize)?,
            unit,
        })
    }
    pub fn completed(&mut self, requested: usize, input: &[u8]) -> Result<&[u8], SpiResultRefusal> {
        if requested == 0 || requested > MAXIMUM_TRANSACTION_BYTES || input.len() != requested {
            return Err(SpiResultRefusal::ActualLength);
        }
        self.encode_completed(input)
            .map_err(SpiResultRefusal::Canonical)
    }
    fn encode_completed(&mut self, input: &[u8]) -> Result<&[u8], StructuredInfoRefusal> {
        let bytes = self.input.leaf(input)?;
        let payload = self
            .completed
            .record(&[validate_canonical_structured_value(bytes)?])?;
        self.result
            .variant("completed", validate_canonical_structured_value(payload)?)
    }
    pub fn disposition(
        &mut self,
        disposition: SpiDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            SpiDisposition::TimedOut => "timed-out",
            SpiDisposition::ProviderLost => "provider-lost",
            SpiDisposition::Unsupported => "unsupported",
            SpiDisposition::StaleAttachment => "stale-attachment",
            SpiDisposition::Refused => "refused",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
