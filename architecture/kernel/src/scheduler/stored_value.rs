//! Private persistent references omit only a store-derived logical byte length.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoredValueKey {
    slot: u16,
    generation: u32,
}
impl StoredValueKey {
    pub(super) fn checked(
        value: ValueRef,
        values: &impl ValueStorage,
    ) -> Result<Self, StorageError> {
        values.reference_count(value)?;
        Ok(Self {
            slot: value.slot,
            generation: value.generation,
        })
    }
    pub(super) fn resolve(self, values: &impl ValueStorage) -> Result<ValueRef, StorageError> {
        values.resolve_identity(self.slot, self.generation)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoredBoundedValue {
    key: StoredValueKey,
    admitted_bytes: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoredHostOutcome {
    disposition: crate::HostCallDisposition,
    output: Option<StoredBoundedValue>,
    failure: Option<crate::Failure>,
}
impl StoredHostOutcome {
    pub(super) fn checked(
        outcome: HostCallOutcome,
        values: &impl ValueStorage,
    ) -> Result<Self, StorageError> {
        let output = outcome
            .output
            .map(|v| {
                Ok(StoredBoundedValue {
                    key: StoredValueKey::checked(v.value, values)?,
                    admitted_bytes: v.admitted_bytes,
                })
            })
            .transpose()?;
        Ok(Self {
            disposition: outcome.disposition,
            output,
            failure: outcome.failure,
        })
    }
    pub(super) fn resolve(
        self,
        values: &impl ValueStorage,
    ) -> Result<HostCallOutcome, StorageError> {
        let output = self
            .output
            .map(|v| {
                Ok(BoundedValueRef {
                    value: v.key.resolve(values)?,
                    admitted_bytes: v.admitted_bytes,
                })
            })
            .transpose()?;
        Ok(HostCallOutcome {
            disposition: self.disposition,
            output,
            failure: self.failure,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_preserves_public_bounds_and_refuses_stale_or_forged_lengths() {
        let mut values = crate::FixedValueStore::<1, 8>::new(8).unwrap();
        values.slots[0].generation = 76_800;
        let value = values.store(b"abc").unwrap();
        let outcome = HostCallOutcome {
            disposition: crate::HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(value, 5).unwrap()),
            failure: None,
        };
        let stored = StoredHostOutcome::checked(outcome, &values).unwrap();
        assert_eq!(stored.resolve(&values), Ok(outcome));
        let mut wrong = outcome;
        wrong.output.as_mut().unwrap().value.byte_len = 2;
        assert_eq!(
            StoredHostOutcome::checked(wrong, &values),
            Err(StorageError::StaleReference)
        );
        values.release(value).unwrap();
        let next = values.store(b"new").unwrap();
        assert_eq!(next.generation, 76_802);
        assert_eq!(stored.resolve(&values), Err(StorageError::StaleReference));
    }
    #[cfg(feature = "alloc")]
    #[test]
    fn identity_resolution_preserves_compact_logical_lengths_and_native_slots() {
        let mut values = crate::HostedValueStore::new(2, 24, 48).unwrap();
        values.prepare_compact_f32();
        let compact = values.store_compact_f32(&1f32.to_le_bytes(), 20).unwrap();
        assert_eq!(
            values.resolve_identity(compact.slot, compact.generation),
            Ok(compact)
        );
        assert_eq!(compact.byte_len, 20);
        let key = StoredValueKey::checked(compact, &values).unwrap();
        assert_eq!(key.resolve(&values), Ok(compact));
        assert_eq!(
            values.get(compact),
            Err(StorageError::RepresentationMismatch)
        );
        let mut native = crate::NativeU64ValueStore::<_, 1>::new(
            crate::FixedValueStore::<1, 1>::new(1).unwrap(),
            8,
        )
        .unwrap();
        let word = native.store_native_u64(42).unwrap();
        assert_eq!(
            native.resolve_identity(word.slot, word.generation),
            Ok(word)
        );
        native.release(word).unwrap();
        assert_eq!(
            native.resolve_identity(word.slot, word.generation),
            Err(StorageError::StaleReference)
        );
    }
}
