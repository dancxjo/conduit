//! Requested payload allocations; root structs, allocator metadata and stack
//! are excluded. Preparation is cumulative requests, hence also a peak bound.
use super::*;
use core::mem::size_of;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedStructuredCompositionStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedStructuredCompositionStorageRefusal {
    Capacity,
    Structured(Refusal),
}
use PreparedStructuredCompositionStorageRefusal as Error;
pub(super) fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Capacity)
}
impl PreparedStructuredComposer {
    /// Allocation-free reservation of every buffer/name/field array constructed
    /// by `new`. Exact array reservations avoid hidden `collect` spare capacity.
    pub fn storage_reservation(
        ty: &StructuredInfoType,
        maximum_bytes: usize,
    ) -> Result<PreparedStructuredCompositionStorageReceipt, Error> {
        let prefix = ty.canonical_byte_length().map_err(Error::Structured)?;
        if maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES || maximum_bytes <= prefix {
            return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
        }
        let mut bytes = add(prefix, maximum_bytes)?;
        match ty.shape() {
            Shape::Leaf(kind) => bytes = add(bytes, kind.as_str().len())?,
            Shape::Record { fields, .. } => {
                bytes = add(
                    bytes,
                    fields
                        .len()
                        .checked_mul(size_of::<Field>())
                        .ok_or(Error::Capacity)?,
                )?;
                for f in fields {
                    bytes = add(
                        add(bytes, f.name().len())?,
                        f.value_type()
                            .canonical_byte_length()
                            .map_err(Error::Structured)?,
                    )?;
                }
            }
            Shape::Variant { cases, .. } => {
                bytes = add(
                    bytes,
                    cases
                        .len()
                        .checked_mul(size_of::<Field>())
                        .ok_or(Error::Capacity)?,
                )?;
                for c in cases {
                    bytes = add(
                        add(bytes, c.tag().len())?,
                        c.payload_type()
                            .canonical_byte_length()
                            .map_err(Error::Structured)?,
                    )?;
                }
            }
            _ => return Err(Error::Structured(Refusal::WrongType)),
        }
        Ok(PreparedStructuredCompositionStorageReceipt {
            preparation_requested_bytes_bound: bytes,
            retained_heap_bytes_bound: bytes,
        })
    }
    pub fn new_with_storage_limits(
        ty: &StructuredInfoType,
        maximum_bytes: usize,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, PreparedStructuredCompositionStorageReceipt), Error> {
        let receipt = Self::storage_reservation(ty, maximum_bytes)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_bytes
        {
            return Err(Error::Capacity);
        }
        Ok((
            Self::new(ty, maximum_bytes).map_err(Error::Structured)?,
            receipt,
        ))
    }
    /// Actual owned capacities, including every spare array/string slot.
    pub fn owned_heap_bytes(&self) -> usize {
        let shape = match &self.shape {
            CompositionShape::Leaf(kind) => kind.capacity(),
            CompositionShape::Record(fields) | CompositionShape::Variant(fields) => {
                fields.iter().fold(
                    fields.capacity().saturating_mul(size_of::<Field>()),
                    |n, f| {
                        n.saturating_add(f.name.capacity())
                            .saturating_add(f.type_bytes.capacity())
                    },
                )
            }
        };
        self.prefix
            .capacity()
            .saturating_add(self.output.capacity())
            .saturating_add(shape)
    }
}
