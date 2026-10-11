//! Prepared exact clock request decoding and bounded result construction.
use super::contract::{CLOCK_MAXIMUM_BYTES, MonotonicClockContract};
use alloc::vec::Vec;
use conduit_core::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockDisposition {
    Unavailable,
    ProviderLost,
    UnsupportedDeadline,
    Malformed,
    Timeout,
}
impl ClockDisposition {
    fn tag(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::ProviderLost => "provider-lost",
            Self::UnsupportedDeadline => "unsupported-deadline",
            Self::Malformed => "malformed",
            Self::Timeout => "timeout",
        }
    }
}

pub struct PreparedClockCodec {
    request: PreparedStructuredValueValidator,
    result: PreparedStructuredComposer,
    instant: PreparedStructuredComposer,
    unit: Vec<u8>,
}
impl PreparedClockCodec {
    pub fn new(contract: &MonotonicClockContract) -> Result<Self, StructuredInfoRefusal> {
        let mut unit = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(kind_id("value/empty"))?,
            64,
        )?;
        Ok(Self {
            request: contract.request_validator()?,
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                CLOCK_MAXIMUM_BYTES as usize,
            )?,
            instant: PreparedStructuredComposer::new(
                &StructuredInfoType::leaf(kind_id("value/u64"))?,
                64,
            )?,
            unit: unit.leaf(&[])?.to_vec(),
        })
    }
    pub fn deadline(&mut self, input: &[u8]) -> Result<u64, StructuredInfoRefusal> {
        self.request.validate(input)?;
        let value = validate_canonical_structured_value(input)?;
        let field = value
            .record_field("deadline")?
            .ok_or(StructuredInfoRefusal::WrongType)?;
        let bytes: [u8; 8] = field
            .primitive_bytes("value/u64")?
            .try_into()
            .map_err(|_| StructuredInfoRefusal::WrongType)?;
        Ok(u64::from_le_bytes(bytes))
    }
    pub fn completed(&mut self, now: u64) -> Result<&[u8], StructuredInfoRefusal> {
        let instant = self.instant.leaf(&now.to_le_bytes())?;
        self.result
            .variant("completed", validate_canonical_structured_value(instant)?)
    }
    pub fn refused(&mut self, reason: ClockDisposition) -> Result<&[u8], StructuredInfoRefusal> {
        self.result.variant(
            reason.tag(),
            validate_canonical_structured_value(&self.unit)?,
        )
    }
}
