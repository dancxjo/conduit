//! Allocation-free decoding after exact schema preparation.

use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

use super::{
    control_contract::ControlContract,
    control_payload::ControlOutputPayload,
    control_request::{ControlRequestRefusal, ControlTransferRequest},
};

#[derive(Debug, PartialEq, Eq)]
pub enum ControlDecodeRefusal {
    Canonical(StructuredInfoRefusal),
    Transfer(ControlRequestRefusal),
}

/// The exact reviewed Type is prepared once. Runtime decoding borrows the
/// canonical input and copies only its bounded payload into fixed storage.
pub struct PreparedControlRequestDecoder {
    validator: PreparedStructuredValueValidator,
}

pub struct DecodedControlRequest {
    setup: [u8; 8],
    output: ControlOutputPayload,
}

impl PreparedControlRequestDecoder {
    pub fn new(contract: &ControlContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }

    pub fn decode(&self, input: &[u8]) -> Result<DecodedControlRequest, ControlDecodeRefusal> {
        self.decode_canonical(input)
            .map_err(ControlDecodeRefusal::Canonical)
            .and_then(|request| {
                request
                    .output
                    .request(request.setup, 256)
                    .map_err(ControlDecodeRefusal::Transfer)?;
                Ok(request)
            })
    }

    fn decode_canonical(
        &self,
        input: &[u8],
    ) -> Result<DecodedControlRequest, StructuredInfoRefusal> {
        self.validator.validate(input)?;
        let value = validate_canonical_structured_value(input)?;
        let setup = value
            .record_field("setup")?
            .ok_or(StructuredInfoRefusal::WrongRecordFields)?
            .primitive_bytes("value/u64")?
            .try_into()
            .map_err(|_| StructuredInfoRefusal::WrongType)?;
        let output = ControlOutputPayload::from_validated(
            value
                .record_field("output")?
                .ok_or(StructuredInfoRefusal::WrongRecordFields)?,
        )?;
        Ok(DecodedControlRequest { setup, output })
    }
}

impl DecodedControlRequest {
    /// The native owner supplies its own actual admitted storage bound here.
    pub fn request(
        &self,
        maximum_data_bytes: u16,
    ) -> Result<ControlTransferRequest<'_>, ControlRequestRefusal> {
        self.output.request(self.setup, maximum_data_bytes)
    }
}

#[cfg(test)]
mod tests;
