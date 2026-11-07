//! Finite typed bus outcomes. No device-specific interpretation or retry policy.
use super::{
    contract::{I2cContract, I2C_MAXIMUM_BYTES},
    transaction::{I2cDisposition, MAXIMUM_TRANSACTION_BYTES},
};
use conduit_core::{
    validate_canonical_structured_value, PreparedStructuredComposer,
    StructuredCompositionPreparationRefusal, StructuredInfoRefusal,
};

pub struct PreparedI2cResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
    input: PreparedStructuredComposer,
    unit: &'static [u8],
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
    }

    pub fn new(_contract: &I2cContract) -> Result<Self, StructuredCompositionPreparationRefusal> {
        let bound = |text: &str| {
            text.parse::<usize>()
                .ok()
                .filter(|bytes| *bytes <= I2C_MAXIMUM_BYTES as usize)
                .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        };
        Ok(Self {
            result: PreparedStructuredComposer::try_from_static_schema(
                include_bytes!(concat!(env!("OUT_DIR"), "/result.type")),
                bound(include_str!(concat!(env!("OUT_DIR"), "/result.bound")))?,
                usize::MAX,
            )?,
            completed: PreparedStructuredComposer::try_from_static_schema(
                include_bytes!(concat!(env!("OUT_DIR"), "/completed.type")),
                bound(include_str!(concat!(env!("OUT_DIR"), "/completed.bound")))?,
                usize::MAX,
            )?,
            short: PreparedStructuredComposer::try_from_static_schema(
                include_bytes!(concat!(env!("OUT_DIR"), "/short.type")),
                bound(include_str!(concat!(env!("OUT_DIR"), "/short.bound")))?,
                usize::MAX,
            )?,
            input: PreparedStructuredComposer::try_from_static_schema(
                include_bytes!(concat!(env!("OUT_DIR"), "/input.type")),
                bound(include_str!(concat!(env!("OUT_DIR"), "/input.bound")))?,
                usize::MAX,
            )?,
            unit: include_bytes!(concat!(env!("OUT_DIR"), "/unit.value")),
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
        let input = self
            .input
            .leaf_collection(input.iter().map(core::slice::from_ref))?;
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
            .variant(tag, validate_canonical_structured_value(self.unit)?)
    }
}
