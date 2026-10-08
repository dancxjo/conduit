//! Fixed collection assembly for canonical parser query fields.
//! The exact prepared Native root supplies the schema; this helper supplies no
//! parser policy and its complete result still requires root Native admission.
use crate::parser_canonical_schema::{select_field, shape, Shape};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollectionRefusal {
    Descriptor,
    Type,
    Pressure,
    Length,
}

pub(crate) struct PreparedParserU64Collection {
    encoded: Vec<u8>,
    values_offset: usize,
    length: usize,
    preparation_requested_bytes_bound: usize,
}
impl PreparedParserU64Collection {
    /// The complete preparation request ceiling is checked before output
    /// allocation. No owned Type decoding is needed. Fixed parser beam widths are at most eight.
    pub(crate) fn prepare<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        field_path: &[&str],
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CollectionRefusal> {
        use CollectionRefusal as R;
        let descriptor = T::PREPARED_DESCRIPTOR;
        if !family.contains_descriptor(descriptor) {
            return Err(R::Descriptor);
        }
        let selected = select_field(descriptor.type_bytes, field_path).map_err(|_| R::Type)?;
        let Shape::Collection { element, length } = shape(selected).map_err(|_| R::Type)? else {
            return Err(R::Type);
        };
        if !(1..=8).contains(&length)
            || !matches!(
                shape(element).map_err(|_| R::Type)?,
                Shape::Leaf("value/u64")
            )
        {
            return Err(R::Type);
        }
        let length = usize::from(length);
        let size = selected
            .len()
            .checked_add(5 + length * 13)
            .ok_or(R::Pressure)?;
        if size > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || size > maximum_preparation_requested_bytes
            || size > maximum_retained_bytes
        {
            return Err(R::Pressure);
        }
        // Schema selection and the entire output request are allocation-free.
        // The fixed output Vec is the only allocation in this constructor.
        let preparation = size;
        let mut encoded = Vec::with_capacity(size);
        encoded.extend_from_slice(selected);
        encoded.push(1);
        encoded.extend_from_slice(&(length as u32).to_le_bytes());
        let values_offset = encoded.len();
        for _ in 0..length {
            encoded.push(0);
            encoded.extend_from_slice(&8u32.to_le_bytes());
            encoded.extend_from_slice(&0u64.to_le_bytes());
        }
        Ok(Self {
            encoded,
            values_offset,
            length,
            preparation_requested_bytes_bound: preparation,
        })
    }
    pub(crate) fn preparation_requested_bytes_bound(&self) -> usize {
        self.preparation_requested_bytes_bound
    }
    pub(crate) fn retained_capacity_bytes(&self) -> usize {
        self.encoded.capacity()
    }
    pub(crate) fn encoded(&self) -> &[u8] {
        &self.encoded
    }
    /// Wrong length preserves the previous output; accepted calls allocate no
    /// storage and preserve all original selected Type and collection framing.
    pub(crate) fn compose(&mut self, values: &[u64]) -> Result<&[u8], CollectionRefusal> {
        if values.len() != self.length {
            return Err(CollectionRefusal::Length);
        }
        for (index, value) in values.iter().enumerate() {
            let start = self.values_offset + index * 13 + 5;
            self.encoded[start..start + 8].copy_from_slice(&value.to_le_bytes());
        }
        Ok(&self.encoded)
    }
}
