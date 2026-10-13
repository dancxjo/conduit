//! Exact nominal framing without reconstructing owned Type or law metadata.
//! The complete result requires generated family admission before use. Nominal
//! identity is preserved; the representation never substitutes for its owner.
use crate::parser_canonical_schema::{select_field, shape, Shape};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NominalRefusal {
    Descriptor,
    Type,
    Pressure,
    Value,
}
pub(crate) struct PreparedParserNominal {
    value_type: &'static [u8],
    representation: &'static [u8],
    encoded: Vec<u8>,
    maximum_bytes: usize,
}
impl PreparedParserNominal {
    /// Borrow the current nominal representation without granting admission.
    pub(crate) fn encoded(&self) -> &[u8] {
        &self.encoded
    }
    /// The single requested buffer capacity is admitted before allocation. All
    /// Type bytes are borrowed from the original exact static descriptor.
    pub(crate) fn prepare<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        field_path: &[&str],
        maximum_encoded_bytes: usize,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_requested_bytes: usize,
    ) -> Result<Self, NominalRefusal> {
        use NominalRefusal as R;
        let descriptor = T::PREPARED_DESCRIPTOR;
        if !family.contains_descriptor(descriptor) {
            return Err(R::Descriptor);
        }
        let value_type = select_field(descriptor.type_bytes, field_path).map_err(|_| R::Type)?;
        let Shape::Nominal { representation } = shape(value_type).map_err(|_| R::Type)? else {
            return Err(R::Type);
        };
        if maximum_encoded_bytes <= value_type.len()
            || maximum_encoded_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_encoded_bytes > maximum_preparation_requested_bytes
            || maximum_encoded_bytes > maximum_retained_requested_bytes
        {
            return Err(R::Pressure);
        }
        let mut encoded = Vec::with_capacity(maximum_encoded_bytes);
        encoded.extend_from_slice(value_type);
        Ok(Self {
            value_type,
            representation,
            encoded,
            maximum_bytes: maximum_encoded_bytes,
        })
    }
    pub(crate) fn requested_bytes_bound(&self) -> usize {
        self.maximum_bytes
    }
    pub(crate) fn retained_capacity_bytes(&self) -> usize {
        self.encoded.capacity()
    }
    pub(crate) fn compose(
        &mut self,
        representation: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<&[u8], NominalRefusal> {
        if representation.type_bytes() != self.representation {
            return Err(NominalRefusal::Type);
        }
        let size = self
            .value_type
            .len()
            .checked_add(representation.value_node().len())
            .ok_or(NominalRefusal::Pressure)?;
        if size > self.maximum_bytes {
            return Err(NominalRefusal::Pressure);
        }
        self.encoded.truncate(self.value_type.len());
        self.encoded.extend_from_slice(representation.value_node());
        validate_canonical_structured_value(&self.encoded).map_err(|_| NominalRefusal::Value)?;
        Ok(&self.encoded)
    }
    /// A nominal leaf still carries its exact outer identity and original leaf
    /// representation. Full canonical validation precedes returned observation.
    pub(crate) fn leaf(&mut self, bytes: &[u8]) -> Result<&[u8], NominalRefusal> {
        if !matches!(shape(self.representation), Ok(Shape::Leaf(_))) {
            return Err(NominalRefusal::Type);
        }
        let size = self
            .value_type
            .len()
            .checked_add(5)
            .and_then(|n| n.checked_add(bytes.len()))
            .ok_or(NominalRefusal::Pressure)?;
        if size > self.maximum_bytes {
            return Err(NominalRefusal::Pressure);
        }
        let length = u32::try_from(bytes.len()).map_err(|_| NominalRefusal::Pressure)?;
        self.encoded.truncate(self.value_type.len());
        self.encoded.push(0);
        self.encoded.extend_from_slice(&length.to_le_bytes());
        self.encoded.extend_from_slice(bytes);
        validate_canonical_structured_value(&self.encoded).map_err(|_| NominalRefusal::Value)?;
        Ok(&self.encoded)
    }
}
