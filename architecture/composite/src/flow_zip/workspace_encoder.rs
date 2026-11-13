//! Exact typed pair composition into the scheduler's one-Step output loan.
use super::*;
use conduit_core::{
    BorrowedStructuredValue, BorrowedStructuredValueValidator, PreparedLinkedStructuredComposer,
};
pub(super) struct WorkspacePair {
    schemas: [Vec<u8>; 2],
    primitive: [bool; 2],
    maxima: [u32; 2],
    output_schema: Vec<u8>,
    output_maximum: u32,
}
impl WorkspacePair {
    pub(super) fn prepare(
        encoder: &PreparedTypedTuplePairEncoder,
        maxima: [u32; 2],
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Record { fields, .. } = encoder.value_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        if fields.len() != 2 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Ok(Self {
            schemas: [
                fields[0].value_type().canonical_bytes()?,
                fields[1].value_type().canonical_bytes()?,
            ],
            primitive: core::array::from_fn(|i| {
                matches!(
                    fields[i].value_type().shape(),
                    StructuredInfoTypeShape::Leaf(_)
                )
            }),
            maxima,
            output_schema: encoder.value_type().canonical_bytes()?,
            output_maximum: encoder.maximum_encoded_bytes(),
        })
    }
    pub(super) fn input_schema(&self, side: usize) -> Option<&[u8]> {
        (!*self.primitive.get(side)?).then_some(self.schemas[side].as_slice())
    }
    pub(super) fn output_schema(&self) -> &[u8] {
        &self.output_schema
    }
    pub(super) fn maximum(&self) -> u32 {
        self.output_maximum
    }
    pub(super) fn encode_into<'out>(
        &self,
        inputs: [&[u8]; 2],
        output: &'out mut [u8],
    ) -> Result<&'out [u8], StructuredInfoRefusal> {
        let member = |side: usize| {
            let input = inputs[side];
            if input.len() > self.maxima[side] as usize {
                return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
            }
            if self.primitive[side] {
                Ok(BorrowedStructuredValue::Leaf(input))
            } else {
                BorrowedStructuredValueValidator::new(
                    &self.schemas[side],
                    self.maxima[side] as usize,
                )?
                .validate(input)
                .map(BorrowedStructuredValue::Canonical)
            }
        };
        let members = [member(0)?, member(1)?];
        PreparedLinkedStructuredComposer::new(&self.output_schema, self.output_maximum as usize)?
            .encode_into(BorrowedStructuredValue::Record(&members), output)
    }
}

#[cfg(test)]
#[path = "workspace_encoder_tests.rs"]
mod tests;
