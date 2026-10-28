//! Fixed value storage and exact first-free slot ownership.
use crate::{StorageError, ValueRef, ValueStorage};

#[derive(Clone, Copy)]
pub(crate) struct FixedValueSlot<const MAX_VALUE_BYTES: usize> {
    pub(crate) generation: u32,
    pub(crate) references: u16,
    pub(crate) len: u32,
    pub(crate) bytes: [u8; MAX_VALUE_BYTES],
}

impl<const MAX_VALUE_BYTES: usize> FixedValueSlot<MAX_VALUE_BYTES> {
    const EMPTY: Self = Self {
        generation: 0,
        references: 0,
        len: 0,
        bytes: [0; MAX_VALUE_BYTES],
    };
}

/// Fixed-storage profile suitable for an embedded static allocation.
pub struct FixedValueStore<const SLOTS: usize, const MAX_VALUE_BYTES: usize> {
    pub(crate) slots: [FixedValueSlot<MAX_VALUE_BYTES>; SLOTS],
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
    // Every earlier slot is occupied or retired. Reusable release rewinds it.
    free_search_start: usize,
    has_retired_slots: bool,
    #[cfg(test)]
    slot_probes: u64,
}

impl<const SLOTS: usize, const MAX_VALUE_BYTES: usize> FixedValueStore<SLOTS, MAX_VALUE_BYTES> {
    /// Borrow only the byte backing of a completely empty store for cold
    /// preparation. Slot identities, generations and ownership never enter the
    /// callback. Bytes are wiped on return and unwind; no ValueRef is created.
    pub fn with_empty_scratch<R>(
        &mut self,
        prepare: impl FnOnce(&mut [&mut [u8]]) -> R,
    ) -> Result<R, StorageError> {
        if self.used_items != 0
            || self.used_bytes != 0
            || self
                .slots
                .iter()
                .any(|slot| slot.references != 0 || slot.len != 0)
        {
            return Err(StorageError::ItemCapacityExceeded);
        }
        struct Scratch<'a, const S: usize, const M: usize>(&'a mut [FixedValueSlot<M>; S]);
        impl<const S: usize, const M: usize> Drop for Scratch<'_, S, M> {
            fn drop(&mut self) {
                for slot in self.0.iter_mut() {
                    slot.bytes.fill(0);
                }
            }
        }
        let guard = Scratch(&mut self.slots);
        for slot in guard.0.iter_mut() {
            slot.bytes.fill(0);
        }
        let mut bytes = guard.0.each_mut().map(|slot| slot.bytes.as_mut_slice());
        Ok(prepare(&mut bytes))
    }

    pub const fn new(byte_capacity: u32) -> Result<Self, StorageError> {
        let physical_bytes = match SLOTS.checked_mul(MAX_VALUE_BYTES) {
            Some(bytes) if bytes <= u32::MAX as usize => bytes as u32,
            _ => return Err(StorageError::InvalidBudget),
        };
        if SLOTS == 0
            || SLOTS > u16::MAX as usize
            || MAX_VALUE_BYTES == 0
            || byte_capacity == 0
            || byte_capacity > physical_bytes
        {
            return Err(StorageError::InvalidBudget);
        }
        Ok(Self {
            slots: [FixedValueSlot::EMPTY; SLOTS],
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
            free_search_start: 0,
            has_retired_slots: false,
            #[cfg(test)]
            slot_probes: 0,
        })
    }

    fn reserve_value_slot(&mut self, length: usize) -> Result<ValueRef, StorageError> {
        if length > MAX_VALUE_BYTES {
            return Err(StorageError::ValueTooLarge);
        }
        let byte_len = u32::try_from(length).map_err(|_| StorageError::ValueTooLarge)?;
        if self
            .used_bytes
            .checked_add(byte_len)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(StorageError::ByteCapacityExceeded);
        }
        #[cfg(test)]
        let probes = &mut self.slot_probes;
        let (offset, slot) = self.slots[self.free_search_start..]
            .iter_mut()
            .enumerate()
            .inspect(|_| {
                #[cfg(test)]
                {
                    *probes += 1;
                }
            })
            .find(|(_, slot)| slot.references == 0 && slot.generation != u32::MAX)
            .ok_or(if self.has_retired_slots {
                StorageError::GenerationExhausted
            } else {
                StorageError::ItemCapacityExceeded
            })?;
        let slot_index = self.free_search_start + offset;
        self.free_search_start = slot_index + 1;
        slot.generation += 1;
        slot.references = 1;
        slot.len = byte_len;
        self.used_items = self
            .used_items
            .checked_add(1)
            .ok_or(StorageError::ItemCapacityExceeded)?;
        self.used_bytes += byte_len;
        Ok(ValueRef {
            slot: u16::try_from(slot_index).map_err(|_| StorageError::ItemCapacityExceeded)?,
            generation: slot.generation,
            byte_len,
        })
    }

    fn slot(&self, value: ValueRef) -> Result<&FixedValueSlot<MAX_VALUE_BYTES>, StorageError> {
        let slot = self
            .slots
            .get(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation || slot.len != value.byte_len
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }

    fn slot_mut(
        &mut self,
        value: ValueRef,
    ) -> Result<&mut FixedValueSlot<MAX_VALUE_BYTES>, StorageError> {
        let slot = self
            .slots
            .get_mut(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation || slot.len != value.byte_len
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }
}

impl<const SLOTS: usize, const MAX_VALUE_BYTES: usize> ValueStorage
    for FixedValueStore<SLOTS, MAX_VALUE_BYTES>
{
    fn item_capacity(&self) -> u16 {
        u16::try_from(SLOTS).unwrap_or(u16::MAX)
    }

    fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }

    fn used_items(&self) -> u16 {
        self.used_items
    }

    fn used_bytes(&self) -> u32 {
        self.used_bytes
    }

    #[cfg(feature = "alloc")]
    fn supports_prepared_encoding(&self) -> bool {
        true
    }
    #[cfg(feature = "alloc")]
    fn store_prepared_encoded(
        &mut self,
        maximum_bytes: usize,
        encode: &mut dyn FnMut(&mut [u8]) -> Result<usize, StorageError>,
    ) -> Result<ValueRef, StorageError> {
        crate::CanonicalEncodedValueStorage::store_encoded(self, maximum_bytes, encode).map_err(
            |error| match error {
                crate::EncodedValueRefusal::Storage(error)
                | crate::EncodedValueRefusal::Encoder(error) => error,
                crate::EncodedValueRefusal::Length => StorageError::ValueTooLarge,
            },
        )
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        let value = self.reserve_value_slot(bytes.len())?;
        self.slots[usize::from(value.slot)].bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(value)
    }

    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        let slot = self.slot(value)?;
        Ok(&slot.bytes[..usize::try_from(slot.len).map_err(|_| StorageError::StaleReference)?])
    }

    fn resolve_identity(&self, slot: u16, generation: u32) -> Result<ValueRef, StorageError> {
        let entry = self
            .slots
            .get(usize::from(slot))
            .ok_or(StorageError::StaleReference)?;
        let value = ValueRef {
            slot,
            generation,
            byte_len: entry.len,
        };
        self.slot(value)?;
        Ok(value)
    }

    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        Ok(self.slot(value)?.references)
    }

    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let slot = self.slot_mut(value)?;
        slot.references = slot
            .references
            .checked_add(1)
            .ok_or(StorageError::ReferenceOverflow)?;
        Ok(())
    }

    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let slot = self.slot_mut(value)?;
        slot.references -= 1;
        if slot.references == 0 {
            let len = slot.len;
            slot.len = 0;
            self.used_items -= 1;
            self.used_bytes -= len;
            if self.slots[usize::from(value.slot)].generation == u32::MAX {
                self.has_retired_slots = true;
            } else {
                self.free_search_start = self.free_search_start.min(usize::from(value.slot));
            }
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            self.has_retired_slots |= slot.generation == u32::MAX;
            slot.references = 0;
            slot.len = 0;
        }
        self.used_items = 0;
        self.used_bytes = 0;
        self.free_search_start = 0;
    }
}

#[cfg(test)]
mod tests;

/// Exclusive borrowed fixed backing retains the same slot generations,
/// canonical byte limits and refusal semantics as its owned store.
impl<const S: usize, const M: usize> ValueStorage for &mut FixedValueStore<S, M> {
    fn item_capacity(&self) -> u16 {
        (**self).item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        (**self).byte_capacity()
    }
    fn used_items(&self) -> u16 {
        (**self).used_items()
    }
    fn used_bytes(&self) -> u32 {
        (**self).used_bytes()
    }
    #[cfg(feature = "alloc")]
    fn supports_prepared_encoding(&self) -> bool {
        (**self).supports_prepared_encoding()
    }
    #[cfg(feature = "alloc")]
    fn store_prepared_encoded(
        &mut self,
        maximum_bytes: usize,
        encode: &mut dyn FnMut(&mut [u8]) -> Result<usize, StorageError>,
    ) -> Result<ValueRef, StorageError> {
        (**self).store_prepared_encoded(maximum_bytes, encode)
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        (**self).store(bytes)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        (**self).get(value)
    }
    fn resolve_identity(&self, slot: u16, generation: u32) -> Result<ValueRef, StorageError> {
        (**self).resolve_identity(slot, generation)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        (**self).reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        (**self).retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        (**self).release(value)
    }
    fn clear(&mut self) {
        (**self).clear()
    }
}

#[cfg(feature = "alloc")]
impl<const S: usize, const M: usize> crate::CanonicalEncodedValueStorage for FixedValueStore<S, M> {
    fn store_encoded<E>(
        &mut self,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, E>,
    ) -> Result<ValueRef, crate::EncodedValueRefusal<E>> {
        let value = self
            .reserve_value_slot(maximum_bytes)
            .map_err(crate::EncodedValueRefusal::Storage)?;
        let mut pending = FixedPending {
            store: self,
            value,
            committed: false,
        };
        let index = usize::from(value.slot);
        pending.store.slots[index].bytes[..maximum_bytes].fill(0);
        let length = encode(&mut pending.store.slots[index].bytes[..maximum_bytes])
            .map_err(crate::EncodedValueRefusal::Encoder)?;
        if length > maximum_bytes {
            return Err(crate::EncodedValueRefusal::Length);
        }
        pending.store.slots[index].bytes[length..maximum_bytes].fill(0);
        pending.store.slots[index].len = length as u32;
        pending.store.used_bytes -= value.byte_len - length as u32;
        pending.value.byte_len = length as u32;
        pending.committed = true;
        Ok(pending.value)
    }
}
#[cfg(feature = "alloc")]
struct FixedPending<'a, const S: usize, const M: usize> {
    store: &'a mut FixedValueStore<S, M>,
    value: ValueRef,
    committed: bool,
}
#[cfg(feature = "alloc")]
impl<const S: usize, const M: usize> Drop for FixedPending<'_, S, M> {
    fn drop(&mut self) {
        if !self.committed {
            self.store.slots[usize::from(self.value.slot)].bytes.fill(0);
            self.store
                .release(self.value)
                .expect("exclusive fixed encoder reservation");
        }
    }
}
#[cfg(feature = "alloc")]
impl<const S: usize, const M: usize> crate::CanonicalEncodedValueStorage
    for &mut FixedValueStore<S, M>
{
    fn store_encoded<E>(
        &mut self,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, E>,
    ) -> Result<ValueRef, crate::EncodedValueRefusal<E>> {
        crate::CanonicalEncodedValueStorage::store_encoded(&mut **self, maximum_bytes, encode)
    }
}
