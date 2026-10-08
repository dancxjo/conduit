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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KindId, StructuredFieldType, StructuredFieldValue, StructuredInfoValue};
    use alloc::{string::String, vec, vec::Vec};
    #[test]
    fn validator_storage_preserves_nested_envelopes_and_refusals() {
        let mut kind = String::with_capacity(96);
        kind.push_str("value/u64");
        let leaf = StructuredInfoType::leaf(KindId::new(kind)).unwrap();
        let nominal =
            StructuredInfoType::nominal(KindId::new("test/nested-number"), leaf.clone()).unwrap();
        let numbers = StructuredInfoType::collection(nominal.clone(), Some(2)).unwrap();
        let mut fields = Vec::with_capacity(12);
        fields.push(StructuredFieldType::new("numbers", numbers.clone()).unwrap());
        let ty = StructuredInfoType::record(KindId::new("test/storage-record"), fields).unwrap();
        let n = |v: u64| {
            StructuredInfoValue::nominal(
                nominal.clone(),
                StructuredInfoValue::leaf(leaf.clone(), v.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap()
        };
        let value = StructuredInfoValue::record(
            ty.clone(),
            vec![StructuredFieldValue::new(
                "numbers",
                StructuredInfoValue::collection(numbers, vec![n(1), n(2)]).unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let encoded = value.canonical_bytes().unwrap();
        let reserve = PreparedStructuredValueValidator::storage_reservation(&ty, 4096).unwrap();
        let (bounded, receipt) = PreparedStructuredValueValidator::new_with_storage_limits(
            &ty,
            4096,
            reserve.preparation_requested_bytes_bound,
            reserve.retained_heap_bytes_bound,
        )
        .unwrap();
        assert_eq!(reserve, receipt);
        assert!(bounded.owned_heap_bytes() <= receipt.retained_heap_bytes_bound);
        assert!(bounded.owned_heap_bytes() < receipt.retained_heap_bytes_bound);
        let ordinary = PreparedStructuredValueValidator::new(&ty, 4096).unwrap();
        for bytes in [&encoded[..], &encoded[..encoded.len() - 1], &[0][..]] {
            assert_eq!(ordinary.validate(bytes), bounded.validate(bytes));
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(ordinary.validate(&trailing), bounded.validate(&trailing));
        assert!(matches!(
            PreparedStructuredValueValidator::new_with_storage_limits(
                &ty,
                4096,
                reserve.preparation_requested_bytes_bound - 1,
                reserve.retained_heap_bytes_bound
            ),
            Err(PreparedStructuredValidationStorageRefusal::Capacity)
        ));
        assert!(matches!(
            PreparedStructuredValueValidator::new_with_storage_limits(
                &ty,
                4096,
                reserve.preparation_requested_bytes_bound,
                reserve.retained_heap_bytes_bound - 1
            ),
            Err(PreparedStructuredValidationStorageRefusal::Capacity)
        ));
        for maximum in [
            0,
            ty.canonical_byte_length().unwrap(),
            MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1,
        ] {
            assert!(matches!(
                PreparedStructuredValueValidator::storage_reservation(&ty, maximum),
                Err(PreparedStructuredValidationStorageRefusal::Structured(
                    Refusal::CanonicalEncodingTooLarge
                ))
            ));
        }
    }
}
