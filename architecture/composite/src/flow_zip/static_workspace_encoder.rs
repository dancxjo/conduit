//! Linked immutable schemas with preparation-checked views, never owned schema copies.
use super::*;
use conduit_core::{
    BorrowedStructuredValue, BorrowedStructuredValueValidator, PreparedLinkedStructuredComposer,
};
pub(super) struct StaticWorkspacePair {
    members: [Option<BorrowedStructuredValueValidator<'static>>; 2],
    maxima: [u32; 2],
    #[cfg(feature = "schema-value-storage")]
    output_schema: &'static [u8],
    composer: PreparedLinkedStructuredComposer<'static>,
    maximum: u32,
}
impl StaticWorkspacePair {
    pub(super) fn prepare(
        encoder: &PreparedTypedTuplePairEncoder,
        maxima: [u32; 2],
        schemas: [&'static [u8]; 3],
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Record { fields, .. } = encoder.value_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        if fields.len() != 2 || encoder.value_type().canonical_bytes()? != schemas[2] {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let mut members = [None; 2];
        for side in 0..2 {
            let ty = fields[side].value_type();
            if ty.canonical_bytes()? != schemas[side] {
                return Err(StructuredInfoRefusal::WrongType);
            }
            if !matches!(ty.shape(), StructuredInfoTypeShape::Leaf(_)) {
                members[side] = Some(BorrowedStructuredValueValidator::new(
                    schemas[side],
                    maxima[side] as usize,
                )?);
            }
        }
        let maximum = encoder.maximum_encoded_bytes();
        Ok(Self {
            members,
            maxima,
            #[cfg(feature = "schema-value-storage")]
            output_schema: schemas[2],
            composer: PreparedLinkedStructuredComposer::new(schemas[2], maximum as usize)?,
            maximum,
        })
    }
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn input_schema(&self, side: usize) -> Option<&[u8]> {
        self.members
            .get(side)?
            .as_ref()
            .map(BorrowedStructuredValueValidator::type_bytes)
    }
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn output_schema(&self) -> &[u8] {
        self.output_schema
    }
    pub(super) fn maximum(&self) -> u32 {
        self.maximum
    }
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn validate_node(
        &self,
        side: usize,
        node: &[u8],
    ) -> Result<(), StructuredInfoRefusal> {
        self.members
            .get(side)
            .and_then(Option::as_ref)
            .ok_or(StructuredInfoRefusal::WrongType)?
            .validate_node(node)
            .map(|_| ())
    }
    pub(super) fn logical_maximum(&self, side: usize) -> usize {
        self.maxima[side] as usize
    }
    pub(super) fn pending_length(&self, side: usize, logical: usize) -> Option<usize> {
        logical.checked_sub(
            self.members
                .get(side)?
                .as_ref()
                .map_or(0, |v| v.type_bytes().len()),
        )
    }
    pub(super) fn input_node<'a>(
        &self,
        side: usize,
        input: &'a [u8],
    ) -> Result<&'a [u8], StructuredInfoRefusal> {
        if input.len() > self.logical_maximum(side) {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        match &self.members[side] {
            Some(validator) => validator.validate(input).map(|value| value.value_node()),
            None => Ok(input),
        }
    }
    #[cfg(test)]
    pub(super) fn encode_into<'out>(
        &self,
        inputs: [&[u8]; 2],
        output: &'out mut [u8],
    ) -> Result<&'out [u8], StructuredInfoRefusal> {
        self.encode_pending_into(
            [
                self.input_node(0, inputs[0])?,
                self.input_node(1, inputs[1])?,
            ],
            output,
        )
    }
    pub(super) fn encode_pending_into<'out>(
        &self,
        inputs: [&[u8]; 2],
        output: &'out mut [u8],
    ) -> Result<&'out [u8], StructuredInfoRefusal> {
        let member = |side: usize| {
            if inputs[side].len() > self.maxima[side] as usize {
                return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
            }
            match &self.members[side] {
                Some(validator) => validator
                    .validate_node(inputs[side])
                    .map(BorrowedStructuredValue::Canonical),
                None => Ok(BorrowedStructuredValue::Leaf(inputs[side])),
            }
        };
        let members = [member(0)?, member(1)?];
        self.composer
            .encode_into(BorrowedStructuredValue::Record(&members), output)
    }
}
#[cfg(test)]
#[path = "static_workspace_encoder_tests.rs"]
mod tests;
