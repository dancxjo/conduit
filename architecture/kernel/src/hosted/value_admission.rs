//! Shared bounded slot admission; no payload representation is materialized here.
use super::*;
impl HostedValueStore {
    pub(super) fn allocate_value_slot(&mut self, length: usize) -> Result<ValueRef, StorageError> {
        if length > self.maximum_value_bytes {
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
        if usize::from(self.used_items) == self.slots.len() {
            return Err(StorageError::ItemCapacityExceeded);
        }
        #[cfg(test)]
        let probes = &mut self.slot_probes;
        let (slot_index, slot) = if self.uniform_slots {
            let start = self.free_search_start;
            self.slots[start..]
                .iter_mut()
                .enumerate()
                .inspect(|_| {
                    #[cfg(test)]
                    {
                        *probes += 1;
                    }
                })
                .find(|(_, slot)| slot.references == 0 && slot.generation != u16::MAX)
                .map(|(offset, slot)| (start + offset, slot))
                .ok_or(if self.has_retired_slots {
                    StorageError::GenerationExhausted
                } else {
                    StorageError::ItemCapacityExceeded
                })?
        } else {
            let mut retired_fit = false;
            let owned = &self.owned_compact_slots;
            self.slots
                .iter_mut()
                .enumerate()
                .inspect(|_| {
                    #[cfg(test)]
                    {
                        *probes += 1;
                    }
                })
                .filter(|(_index, slot)| {
                    let free_fit = slot.references == 0
                        && slot.bytes.capacity() >= length
                        && owned.get(*_index).is_none_or(Option::is_none);
                    retired_fit |= free_fit && slot.generation == u16::MAX;
                    free_fit && slot.generation != u16::MAX
                })
                .min_by_key(|(_, slot)| slot.bytes.capacity())
                .ok_or(if retired_fit {
                    StorageError::GenerationExhausted
                } else {
                    StorageError::ItemCapacityExceeded
                })?
        };
        if self.uniform_slots {
            self.free_search_start = slot_index + 1;
        }
        slot.generation += 1;
        slot.references = 1;
        slot.bytes.clear();
        debug_assert!(slot.bytes.capacity() >= length);
        self.used_items += 1;
        self.used_bytes += byte_len;
        Ok(ValueRef {
            slot: u16::try_from(slot_index).map_err(|_| StorageError::ItemCapacityExceeded)?,
            generation: slot.generation,
            byte_len,
        })
    }
}
