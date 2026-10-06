//! Exact received extent and native dispositions; no class interpretation.
use super::endpoint_read_contract::{
    ENDPOINT_READ_DATA_BYTES, ENDPOINT_READ_MAXIMUM_BYTES, EndpointReadContract,
};
use alloc::vec::Vec;
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    kind_id, validate_canonical_structured_value,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EndpointReadDisposition {
    Stalled,
    ProviderLost,
    Unsupported,
    Timeout,
}
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointReadResultRefusal {
    Canonical(StructuredInfoRefusal),
    DataEnvelope,
    ActualLength,
    InputLength,
}

pub struct PreparedEndpointReadResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    wire: PreparedStructuredComposer,
    ordinal: PreparedStructuredComposer,
    actual: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
    unit: Vec<u8>,
}
impl PreparedEndpointReadResultEncoder {
    pub fn new(contract: &EndpointReadContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let completed = cases
            .iter()
            .find(|c| c.tag() == "completed")
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?
            .payload_type();
        let primitive = |name| StructuredInfoType::leaf(kind_id(name));
        let mut unit = PreparedStructuredComposer::new(&primitive("value/unit")?, 64)?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                ENDPOINT_READ_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(
                completed,
                ENDPOINT_READ_MAXIMUM_BYTES as usize,
            )?,
            wire: PreparedStructuredComposer::new(
                &primitive("value/bytes")?,
                ENDPOINT_READ_MAXIMUM_BYTES as usize,
            )?,
            ordinal: PreparedStructuredComposer::new(&primitive("value/u64")?, 64)?,
            actual: PreparedStructuredComposer::new(&primitive("value/u64")?, 64)?,
            short: PreparedStructuredComposer::new(&primitive("value/bool")?, 64)?,
            unit,
        })
    }
    pub fn completed(
        &mut self,
        ordinal: u64,
        requested: u16,
        actual: u16,
        input: &[u8],
    ) -> Result<&[u8], EndpointReadResultRefusal> {
        use EndpointReadResultRefusal as Error;
        if requested == 0
            || requested > ENDPOINT_READ_DATA_BYTES
            || input.len() > usize::from(ENDPOINT_READ_DATA_BYTES)
        {
            return Err(Error::DataEnvelope);
        }
        if actual > requested {
            return Err(Error::ActualLength);
        }
        if input.len() != usize::from(actual) {
            return Err(Error::InputLength);
        }
        let ordinal_value = self
            .ordinal
            .leaf(&ordinal.to_le_bytes())
            .map_err(Error::Canonical)?;
        let wire = self.wire.leaf(input).map_err(Error::Canonical)?;
        let actual_value = self
            .actual
            .leaf(&u64::from(actual).to_le_bytes())
            .map_err(Error::Canonical)?;
        let short = self
            .short
            .leaf(&[u8::from(actual < requested)])
            .map_err(Error::Canonical)?;
        let completed = self
            .completed
            .record(&[
                validate_canonical_structured_value(ordinal_value).map_err(Error::Canonical)?,
                validate_canonical_structured_value(actual_value).map_err(Error::Canonical)?,
                validate_canonical_structured_value(short).map_err(Error::Canonical)?,
                validate_canonical_structured_value(wire).map_err(Error::Canonical)?,
            ])
            .map_err(Error::Canonical)?;
        self.result
            .variant(
                "completed",
                validate_canonical_structured_value(completed).map_err(Error::Canonical)?,
            )
            .map_err(Error::Canonical)
    }
    pub fn disposition(
        &mut self,
        disposition: EndpointReadDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            EndpointReadDisposition::Stalled => "stalled",
            EndpointReadDisposition::ProviderLost => "provider-lost",
            EndpointReadDisposition::Unsupported => "unsupported",
            EndpointReadDisposition::Timeout => "timeout",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
