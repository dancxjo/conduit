//! Encode directly into an admitted canonical slot, avoiding a second buffer.
use super::*;
#[derive(Debug, PartialEq, Eq)]
pub enum EncodedValueRefusal<E> {
    Storage(StorageError),
    Encoder(E),
    Length,
}
impl HostedValueStore {
    /// Reserve the complete finite byte ceiling before invoking the encoder.
    /// The encoder can only write that free slot. Success publishes its actual
    /// length; error/unwind wipes and releases the reservation. An attempted
    /// publication consumes a slot generation even when encoding fails, so no
    /// stale reference can be revived. No capacity grows during this operation.
    pub fn store_encoded<E>(
        &mut self,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, E>,
    ) -> Result<ValueRef, EncodedValueRefusal<E>> {
        let value = self
            .allocate_value_slot(maximum_bytes)
            .map_err(EncodedValueRefusal::Storage)?;
        let mut pending = Pending {
            store: self,
            value,
            committed: false,
        };
        let index = usize::from(value.slot);
        pending.store.slots[index].bytes.resize(maximum_bytes, 0);
        let length =
            encode(&mut pending.store.slots[index].bytes).map_err(EncodedValueRefusal::Encoder)?;
        if length > maximum_bytes {
            return Err(EncodedValueRefusal::Length);
        }
        // The reservation already admitted this larger logical extent. These
        // operations cannot fail or allocate after the encoder has returned.
        let length = length as u32;
        pending.store.slots[index].bytes[length as usize..].fill(0);
        pending.store.slots[index].bytes.truncate(length as usize);
        pending.store.used_bytes -= value.byte_len - length;
        pending.value.byte_len = length;
        if let Some(word) = pending.store.native_words.get_mut(index) {
            *word = None;
        }
        pending.store.clear_compact_layout(value.slot);
        pending.committed = true;
        Ok(pending.value)
    }
}
struct Pending<'a> {
    store: &'a mut HostedValueStore,
    value: ValueRef,
    committed: bool,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.store.slots[usize::from(self.value.slot)].bytes.fill(0);
            // Only the encoder's slice was exposed; it cannot change the
            // reservation's ownership, generation or admitted logical quota.
            self.store
                .release(self.value)
                .expect("owned encoding reservation");
        }
    }
}
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn encoding_uses_the_reserved_slot_and_publishes_only_actual_length() {
        let mut store = HostedValueStore::new_with_slot_capacities(&[8, 16], 24).unwrap();
        let value = store
            .store_encoded(8, |out| {
                out[..3].copy_from_slice(b"abc");
                Ok::<_, ()>(3)
            })
            .unwrap();
        assert_eq!(store.get(value).unwrap(), b"abc");
        assert_eq!((store.used_items(), store.used_bytes()), (1, 3));
        assert_eq!(store.reference_count(value).unwrap(), 1);
        let second = store
            .store_encoded(16, |out| {
                out.fill(7);
                Ok::<_, ()>(16)
            })
            .unwrap();
        assert_eq!(second.slot, 1);
        assert_eq!(store.used_bytes(), 19);
        store.release(value).unwrap();
        store.release(second).unwrap();
        assert_eq!((store.used_items(), store.used_bytes()), (0, 0));
        assert_eq!(store.slots[0].bytes.capacity(), 8);
        assert_eq!(store.slots[1].bytes.capacity(), 16);
    }
    #[test]
    fn encoder_failure_and_invalid_lengths_wipe_and_release_without_reviving_generations() {
        let mut store = HostedValueStore::new(1, 8, 8).unwrap();
        let old = store.store(b"prior").unwrap();
        store.release(old).unwrap();
        assert_eq!(
            store.store_encoded(8, |out| {
                out.fill(9);
                Err::<usize, _>(7)
            }),
            Err(EncodedValueRefusal::Encoder(7))
        );
        assert_eq!((store.used_items(), store.used_bytes()), (0, 0));
        assert_eq!(
            store.store_encoded(8, |_| Ok::<_, ()>(9)),
            Err(EncodedValueRefusal::Length)
        );
        assert_eq!((store.used_items(), store.used_bytes()), (0, 0));
        let new = store
            .store_encoded(8, |out| {
                assert_eq!(out, &[0; 8]);
                Ok::<_, ()>(0)
            })
            .unwrap();
        assert_ne!(new.generation, old.generation);
        assert_eq!(store.get(old), Err(StorageError::StaleReference));
        assert_eq!(store.get(new).unwrap(), b"");
    }
    #[test]
    fn reserve_refuses_before_calling_the_encoder_and_preserves_other_values() {
        let mut store = HostedValueStore::new(1, 8, 8).unwrap();
        let held = store.store(b"retained").unwrap();
        assert_eq!(
            store.store_encoded(9, |_| panic!("must not run")),
            Err::<ValueRef, EncodedValueRefusal<()>>(EncodedValueRefusal::Storage(
                StorageError::ValueTooLarge
            ))
        );
        assert_eq!(
            store.store_encoded(8, |_| panic!("must not run")),
            Err::<ValueRef, EncodedValueRefusal<()>>(EncodedValueRefusal::Storage(
                StorageError::ByteCapacityExceeded
            ))
        );
        assert_eq!(store.get(held).unwrap(), b"retained");
        assert_eq!(store.reference_count(held).unwrap(), 1);
    }
    #[test]
    fn unwind_releases_the_private_reservation() {
        let mut store = HostedValueStore::new(1, 8, 8).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.store_encoded::<()>(8, |out| {
                out.fill(3);
                panic!("encoder failure")
            });
        }))
        .is_err());
        assert_eq!((store.used_items(), store.used_bytes()), (0, 0));
        store
            .store_encoded(8, |out| {
                assert_eq!(out, &[0; 8]);
                Ok::<_, ()>(0)
            })
            .unwrap();
    }
}
