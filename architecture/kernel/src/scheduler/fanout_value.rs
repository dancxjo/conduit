//! Prepare one new value's complete reference fan-out before publishing it.
use crate::{StorageError, ValueRef, ValueStorage};

pub(super) fn store_fanout_value(
    values: &mut impl ValueStorage,
    bytes: &[u8],
    references: usize,
) -> Result<ValueRef, StorageError> {
    if references == 0 {
        return Err(StorageError::InvalidBudget);
    }
    if references > usize::from(u16::MAX) {
        return Err(StorageError::ReferenceOverflow);
    }
    let value = values.store(bytes)?;
    for retained in 1..references {
        if let Err(error) = values.retain(value) {
            // The new value has not escaped. Release its original reference
            // and every successful retain, leaving no unpublished ownership.
            for _ in 0..retained {
                values.release(value)?;
            }
            return Err(error);
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixedValueStore;

    #[test]
    fn invalid_fanout_count_refuses_before_storage_mutation() {
        let mut values = FixedValueStore::<2, 1>::new(2).unwrap();
        let existing = values.store(b"x").unwrap();
        for (count, error) in [
            (0, StorageError::InvalidBudget),
            (usize::from(u16::MAX) + 1, StorageError::ReferenceOverflow),
        ] {
            assert_eq!(store_fanout_value(&mut values, b"y", count), Err(error));
            assert_eq!(values.used_items(), 1);
            assert_eq!(values.used_bytes(), 1);
            assert_eq!(values.reference_count(existing), Ok(1));
        }
    }

    #[test]
    fn maximum_representable_fanout_preserves_exact_reference_count() {
        let mut values = FixedValueStore::<1, 1>::new(1).unwrap();
        let value = store_fanout_value(&mut values, b"x", usize::from(u16::MAX)).unwrap();
        assert_eq!(values.reference_count(value), Ok(u16::MAX));
        for _ in 0..u16::MAX {
            values.release(value).unwrap();
        }
        assert_eq!(values.used_items(), 0);
    }
}
