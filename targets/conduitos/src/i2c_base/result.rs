//! Finite typed bus outcomes. No device-specific interpretation or retry policy.
use super::{
    contract::{I2C_MAXIMUM_BYTES, I2cContract},
    transaction::{I2cDisposition, MAXIMUM_TRANSACTION_BYTES},
};
use alloc::vec::Vec;
use conduit_core::{
    PreparedLeafSequenceEncoder, PreparedStructuredComposer, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoTypeShape, kind_id, validate_canonical_structured_value,
};

pub struct PreparedI2cResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
    input: PreparedLeafSequenceEncoder,
    unit: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum I2cResultRefusal {
    Canonical(StructuredInfoRefusal),
    ActualLength,
}

impl PreparedI2cResultEncoder {
    pub fn new(contract: &I2cContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let payload = |tag| {
            cases
                .iter()
                .find(|case| case.tag() == tag)
                .map(|case| case.payload_type())
                .ok_or(StructuredInfoRefusal::UnknownVariantTag)
        };
        let mut unit =
            PreparedStructuredComposer::new(&StructuredInfoType::leaf(kind_id("value/unit"))?, 64)?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                I2C_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(
                payload("completed")?,
                I2C_MAXIMUM_BYTES as usize,
            )?,
            short: PreparedStructuredComposer::new(payload("short")?, I2C_MAXIMUM_BYTES as usize)?,
            input: PreparedLeafSequenceEncoder::new(
                kind_id("value/u8"),
                1,
                MAXIMUM_TRANSACTION_BYTES as u16,
            )?,
            unit,
        })
    }
    pub fn completed(&mut self, requested: u8, input: &[u8]) -> Result<&[u8], I2cResultRefusal> {
        if usize::from(requested) > MAXIMUM_TRANSACTION_BYTES
            || input.len() > usize::from(requested)
        {
            return Err(I2cResultRefusal::ActualLength);
        }
        self.encode_completed(requested, input)
            .map_err(I2cResultRefusal::Canonical)
    }
    fn encode_completed(
        &mut self,
        requested: u8,
        input: &[u8],
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let short = input.len() < usize::from(requested);
        let input = self.input.encode(input.iter().map(core::slice::from_ref))?;
        let payload = if short {
            &mut self.short
        } else {
            &mut self.completed
        };
        let payload = payload.record(&[validate_canonical_structured_value(input)?])?;
        self.result.variant(
            if short { "short" } else { "completed" },
            validate_canonical_structured_value(payload)?,
        )
    }
    pub fn disposition(
        &mut self,
        disposition: I2cDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            I2cDisposition::DeviceError => "device-error",
            I2cDisposition::NotAcknowledged => "not-acknowledged",
            I2cDisposition::ArbitrationLost => "arbitration-lost",
            I2cDisposition::TimedOut => "timed-out",
            I2cDisposition::ProviderLost => "provider-lost",
            I2cDisposition::Unsupported => "unsupported",
            I2cDisposition::StaleAttachment => "stale-attachment",
            I2cDisposition::Refused => "refused",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
