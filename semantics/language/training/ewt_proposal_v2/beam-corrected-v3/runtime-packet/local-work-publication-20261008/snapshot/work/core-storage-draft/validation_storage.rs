//! Explicit requested heap accounting for validator preparation.
use super::{
    PreparedStructuredValueValidator, Refusal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedStructuredValidationStorageReceipt {
    /// Conservative cumulative allocation requests during preparation.
    pub preparation_requested_bytes_bound: usize,
    /// Retained heap requests; root struct and allocator bookkeeping excluded.
    pub retained_heap_bytes_bound: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedStructuredValidationStorageRefusal {
    Capacity,
    Structured(Refusal),
}
impl PreparedStructuredValueValidator {
    /// Allocation-free reservation for the unchanged `new` implementation.
    /// Source spare capacities conservatively overcount fresh Type cloning.
    pub fn storage_reservation(
        value_type: &StructuredInfoType,
        maximum_bytes: usize,
    ) -> Result<
        PreparedStructuredValidationStorageReceipt,
        PreparedStructuredValidationStorageRefusal,
    > {
        use PreparedStructuredValidationStorageRefusal as Error;
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
        }
        let prefix = value_type
            .canonical_byte_length()
            .map_err(Error::Structured)?;
        if prefix >= maximum_bytes {
            return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
        }
        let owned = value_type.owned_heap_bytes();
        if owned == usize::MAX {
            return Err(Error::Capacity);
        }
        let bound = owned.checked_add(prefix).ok_or(Error::Capacity)?;
        Ok(PreparedStructuredValidationStorageReceipt {
            preparation_requested_bytes_bound: bound,
            retained_heap_bytes_bound: bound,
        })
    }
    /// Refuses both storage ceilings before Type cloning or prefix allocation.
    /// Validation predicates and the ordinary `new` entrance are unchanged.
    pub fn new_with_storage_limits(
        value_type: &StructuredInfoType,
        maximum_bytes: usize,
        maximum_preparation_peak_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<
        (Self, PreparedStructuredValidationStorageReceipt),
        PreparedStructuredValidationStorageRefusal,
    > {
        use PreparedStructuredValidationStorageRefusal as Error;
        let receipt = Self::storage_reservation(value_type, maximum_bytes)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_peak_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_bytes
        {
            return Err(Error::Capacity);
        }
        let validator = Self::new(value_type, maximum_bytes).map_err(Error::Structured)?;
        Ok((validator, receipt))
    }
    /// Actual requested heap capacities; excludes the root struct and allocator bookkeeping.
    pub fn owned_heap_bytes(&self) -> usize {
        self.value_type
            .owned_heap_bytes()
            .saturating_add(self.prefix.capacity())
    }
}
