//! Canonical actual OUT extent and acknowledged native dispositions.
use super::endpoint_write_contract::{
    ENDPOINT_WRITE_DATA_BYTES, ENDPOINT_WRITE_MAXIMUM_BYTES, EndpointWriteContract,
};
use alloc::vec::Vec;
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    kind_id, validate_canonical_structured_value,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EndpointWriteDisposition {
    Stalled,
    ProviderLost,
    Unsupported,
}
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointWriteResultRefusal {
    Canonical(StructuredInfoRefusal),
    DataEnvelope,
    ActualLength,
}
pub struct PreparedEndpointWriteResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    actual: PreparedStructuredComposer,
    unit: Vec<u8>,
}
impl PreparedEndpointWriteResultEncoder {
    pub fn new(contract: &EndpointWriteContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let completed = cases
            .iter()
            .find(|c| c.tag() == "completed")
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?
            .payload_type();
        let primitive = |name| StructuredInfoType::leaf(kind_id(name));
        let mut unit = PreparedStructuredComposer::new(&primitive("value/empty")?, 64)?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                ENDPOINT_WRITE_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(
                completed,
                ENDPOINT_WRITE_MAXIMUM_BYTES as usize,
            )?,
            actual: PreparedStructuredComposer::new(&primitive("value/u64")?, 64)?,
            unit,
        })
    }
    pub fn completed(
        &mut self,
        requested: u16,
        actual: u16,
    ) -> Result<&[u8], EndpointWriteResultRefusal> {
        use EndpointWriteResultRefusal as Error;
        if requested > ENDPOINT_WRITE_DATA_BYTES {
            return Err(Error::DataEnvelope);
        }
        if actual > requested {
            return Err(Error::ActualLength);
        }
        let actual_value = self
            .actual
            .leaf(&u64::from(actual).to_le_bytes())
            .map_err(Error::Canonical)?;
        let completed = self
            .completed
            .record(&[validate_canonical_structured_value(actual_value).map_err(Error::Canonical)?])
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
        disposition: EndpointWriteDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            EndpointWriteDisposition::Stalled => "stalled",
            EndpointWriteDisposition::ProviderLost => "provider-lost",
            EndpointWriteDisposition::Unsupported => "unsupported",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
#[cfg(test)]
mod tests;
