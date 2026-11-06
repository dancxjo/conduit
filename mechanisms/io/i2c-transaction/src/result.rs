//! Finite typed bus outcomes. No device-specific interpretation or retry policy.
use super::{
    contract::I2cContract,
    transaction::{I2cDisposition, MAXIMUM_TRANSACTION_BYTES},
};
use alloc::vec::Vec;
use conduit_core::{
    kind_id, validate_canonical_structured_value, PreparedLeafSequenceEncoder,
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
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
    /// Retained buffer/schema storage; does not grant execution memory.
    pub fn prepared_heap_bytes(&self) -> usize {
        self.result.prepared_heap_bytes()
            + self.completed.prepared_heap_bytes()
            + self.short.prepared_heap_bytes()
            + self.input.prepared_heap_bytes()
            + self.unit.capacity()
    }

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
        let mut unit = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(kind_id("value/empty"))?,
            64,
        )?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::with_static_schema(
                contract.result_type(),
                include_bytes!(concat!(env!("OUT_DIR"), "/result.type")),
                include_str!(concat!(env!("OUT_DIR"), "/result.bound"))
                    .parse()
                    .map_err(|_| StructuredInfoRefusal::WrongType)?,
            )?,
            completed: PreparedStructuredComposer::with_static_schema(
                payload("completed")?,
                include_bytes!(concat!(env!("OUT_DIR"), "/completed.type")),
                include_str!(concat!(env!("OUT_DIR"), "/completed.bound"))
                    .parse()
                    .map_err(|_| StructuredInfoRefusal::WrongType)?,
            )?,
            short: PreparedStructuredComposer::with_static_schema(
                payload("short")?,
                include_bytes!(concat!(env!("OUT_DIR"), "/short.type")),
                include_str!(concat!(env!("OUT_DIR"), "/short.bound"))
                    .parse()
                    .map_err(|_| StructuredInfoRefusal::WrongType)?,
            )?,
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
