//! Typed native transfer observations, without descriptor or class policy.

use alloc::vec::Vec;
use conduit_core::{
    PreparedLeafSequenceEncoder, PreparedStructuredComposer, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoTypeShape, kind_id, validate_canonical_structured_value,
};

use super::{
    control_contract::{CONTROL_MAXIMUM_BYTES, ControlContract},
    control_request::ControlTransferRequest,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ControlTransferDisposition {
    Stalled,
    ProviderLost,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ControlResultRefusal {
    Canonical(StructuredInfoRefusal),
    DataEnvelope,
    ActualLength,
    InputLength,
    UnexpectedInputData,
}

pub struct PreparedControlResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    input: PreparedLeafSequenceEncoder,
    short: PreparedStructuredComposer,
    transferred: PreparedStructuredComposer,
    unit: Vec<u8>,
}

impl PreparedControlResultEncoder {
    pub fn new(contract: &ControlContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let completed = cases
            .iter()
            .find(|case| case.tag() == "completed")
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?
            .payload_type();
        let primitive = |identity| StructuredInfoType::leaf(kind_id(identity));
        let mut unit = PreparedStructuredComposer::new(&primitive("value/empty")?, 64)?;
        let unit = unit.leaf(&[])?.to_vec(); // preparation only
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                CONTROL_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(completed, CONTROL_MAXIMUM_BYTES as usize)?,
            input: PreparedLeafSequenceEncoder::new(kind_id("value/u8"), 1, 256)?,
            short: PreparedStructuredComposer::new(&primitive("value/bool")?, 64)?,
            transferred: PreparedStructuredComposer::new(&primitive("value/u16")?, 64)?,
            unit,
        })
    }

    pub fn completed(
        &mut self,
        request: &ControlTransferRequest<'_>,
        actual: u16,
        input: &[u8],
    ) -> Result<&[u8], ControlResultRefusal> {
        if request.length() > 256 || input.len() > 256 {
            return Err(ControlResultRefusal::DataEnvelope);
        }
        if actual > request.length() || (!request.input() && actual != request.length()) {
            return Err(ControlResultRefusal::ActualLength);
        }
        if request.input() {
            if input.len() != usize::from(actual) {
                return Err(ControlResultRefusal::InputLength);
            }
        } else if !input.is_empty() {
            return Err(ControlResultRefusal::UnexpectedInputData);
        }
        self.encode_completed(request.length(), actual, input)
            .map_err(ControlResultRefusal::Canonical)
    }

    fn encode_completed(
        &mut self,
        requested: u16,
        actual: u16,
        input: &[u8],
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let input = self.input.encode(input.iter().map(core::slice::from_ref))?;
        let short = self.short.leaf(&[u8::from(actual < requested)])?;
        let transferred = self.transferred.leaf(&actual.to_le_bytes())?;
        let completed = self.completed.record(&[
            validate_canonical_structured_value(input)?,
            validate_canonical_structured_value(short)?,
            validate_canonical_structured_value(transferred)?,
        ])?;
        self.result
            .variant("completed", validate_canonical_structured_value(completed)?)
    }

    pub fn disposition(
        &mut self,
        disposition: ControlTransferDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            ControlTransferDisposition::Stalled => "stalled",
            ControlTransferDisposition::ProviderLost => "provider-lost",
            ControlTransferDisposition::Unsupported => "unsupported",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}

#[cfg(test)]
mod tests;
