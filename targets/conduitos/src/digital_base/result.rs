//! Prepared, bounded digital outcomes. No partial result can masquerade as completion.
use super::{
    DigitalDisposition,
    contract::{DIGITAL_MAXIMUM_BYTES, DigitalContract},
};
use alloc::vec::Vec;
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    kind_id, validate_canonical_structured_value,
};

pub struct PreparedDigitalResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    input: PreparedStructuredComposer,
    unit: Vec<u8>,
}

impl PreparedDigitalResultEncoder {
    pub fn new(contract: &DigitalContract) -> Result<Self, StructuredInfoRefusal> {
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
            .find(|field| field.name() == "high")
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
                DIGITAL_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(completed, DIGITAL_MAXIMUM_BYTES as usize)?,
            input: PreparedStructuredComposer::new(input, DIGITAL_MAXIMUM_BYTES as usize)?,
            unit,
        })
    }
    pub fn completed(&mut self, high: bool) -> Result<&[u8], StructuredInfoRefusal> {
        let bytes = self.input.leaf(&[u8::from(high)])?;
        let payload = self
            .completed
            .record(&[validate_canonical_structured_value(bytes)?])?;
        self.result
            .variant("completed", validate_canonical_structured_value(payload)?)
    }
    pub fn disposition(
        &mut self,
        disposition: DigitalDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            DigitalDisposition::ProviderLost => "provider-lost",
            DigitalDisposition::Unsupported => "unsupported",
            DigitalDisposition::Refused => "refused",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
