use alloc::vec::Vec;

use super::{StorageError, ValueRef, ValueStorage};

struct HostedValueSlot {
    generation: u16,
    references: u16,
    bytes: Vec<u8>,
}

/// Hosted profile that allocates every slot and byte ceiling before use.
/// Once constructed, `store` never grows a vector.
pub struct HostedValueStore {
    slots: Vec<HostedValueSlot>,
    maximum_value_bytes: usize,
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
    // Capacities never change after preparation. Equal best fits are first-free.
    uniform_slots: bool,
    // Earlier uniform slots are occupied or permanently retired.
    free_search_start: usize,
    has_retired_slots: bool,
    #[cfg(test)]
    slot_probes: u64,
}

impl HostedValueStore {
    pub fn new(
        item_capacity: u16,
        maximum_value_bytes: u32,
        byte_capacity: u32,
    ) -> Result<Self, StorageError> {
        Self::from_capacities(
            (0..usize::from(item_capacity)).map(|_| maximum_value_bytes),
            byte_capacity,
        )
    }

    /// Prepare independently bounded slots. The live-byte quota may be smaller
    /// than the backing storage reserved before Play.
    pub fn new_with_slot_capacities(
        capacities: &[u32],
        byte_capacity: u32,
    ) -> Result<Self, StorageError> {
        Self::from_capacities(capacities.iter().copied(), byte_capacity)
    }

    fn from_capacities(
        capacities: impl ExactSizeIterator<Item = u32> + Clone,
        byte_capacity: u32,
    ) -> Result<Self, StorageError> {
        if capacities.len() == 0 || capacities.len() > usize::from(u16::MAX) || byte_capacity == 0 {
            return Err(StorageError::InvalidBudget);
        }
        let mut reserved = 0_u32;
        let mut maximum_value_bytes = 0_u32;
        for capacity in capacities.clone() {
            if capacity == 0 {
                return Err(StorageError::InvalidBudget);
            }
            reserved = reserved
                .checked_add(capacity)
                .ok_or(StorageError::InvalidBudget)?;
            maximum_value_bytes = maximum_value_bytes.max(capacity);
        }
        if byte_capacity > reserved {
            return Err(StorageError::InvalidBudget);
        }
        let mut slots = Vec::with_capacity(capacities.len());
        for capacity in capacities {
            slots.push(HostedValueSlot {
                generation: 0,
                references: 0,
                bytes: Vec::with_capacity(capacity as usize),
            });
        }
        let uniform_slots = slots
            .windows(2)
            .all(|pair| pair[0].bytes.capacity() == pair[1].bytes.capacity());
        Ok(Self {
            slots,
            uniform_slots,
            free_search_start: 0,
            has_retired_slots: false,
            maximum_value_bytes: maximum_value_bytes as usize,
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
            #[cfg(test)]
            slot_probes: 0,
        })
    }

    /// Actual reserved backing storage, including slot metadata. This differs
    /// from `byte_capacity`, which bounds live payload bytes.
    pub fn reserved_storage_bytes(&self) -> usize {
        self.slots.capacity() * core::mem::size_of::<HostedValueSlot>()
            + self
                .slots
                .iter()
                .map(|slot| slot.bytes.capacity())
                .sum::<usize>()
    }

    pub fn allocation_capacities(&self) -> (usize, usize) {
        (
            self.slots.capacity(),
            self.slots.iter().map(|slot| slot.bytes.capacity()).sum(),
        )
    }

    fn slot(&self, value: ValueRef) -> Result<&HostedValueSlot, StorageError> {
        let slot = self
            .slots
            .get(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0
            || slot.generation != value.generation
            || slot.bytes.len() != usize::try_from(value.byte_len).unwrap_or(usize::MAX)
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, value: ValueRef) -> Result<&mut HostedValueSlot, StorageError> {
        let slot = self
            .slots
            .get_mut(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0
            || slot.generation != value.generation
            || slot.bytes.len() != usize::try_from(value.byte_len).unwrap_or(usize::MAX)
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }
}

impl ValueStorage for HostedValueStore {
    fn item_capacity(&self) -> u16 {
        u16::try_from(self.slots.len()).unwrap_or(u16::MAX)
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

    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        if bytes.len() > self.maximum_value_bytes {
            return Err(StorageError::ValueTooLarge);
        }
        let byte_len = u32::try_from(bytes.len()).map_err(|_| StorageError::ValueTooLarge)?;
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
            self.slots
                .iter_mut()
                .enumerate()
                .inspect(|_| {
                    #[cfg(test)]
                    {
                        *probes += 1;
                    }
                })
                .filter(|(_, slot)| {
                    let free_fit = slot.references == 0 && slot.bytes.capacity() >= bytes.len();
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
        slot.bytes.extend_from_slice(bytes);
        debug_assert!(slot.bytes.capacity() >= bytes.len());
        self.used_items += 1;
        self.used_bytes += byte_len;
        Ok(ValueRef {
            slot: u16::try_from(slot_index).map_err(|_| StorageError::ItemCapacityExceeded)?,
            generation: slot.generation,
            byte_len,
        })
    }

    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        Ok(self.slot(value)?.bytes.as_slice())
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
            let len = u32::try_from(slot.bytes.len()).unwrap_or(u32::MAX);
            slot.bytes.clear();
            self.used_items -= 1;
            self.used_bytes -= len;
            if self.slots[usize::from(value.slot)].generation == u16::MAX {
                self.has_retired_slots = true;
            } else if self.uniform_slots {
                self.free_search_start = self.free_search_start.min(usize::from(value.slot));
            }
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            self.has_retired_slots |= slot.generation == u16::MAX;
            slot.references = 0;
            slot.bytes.clear();
        }
        self.used_items = 0;
        self.used_bytes = 0;
        self.free_search_start = 0;
    }
}

pub use HostedValueStore as Store;

#[cfg(test)]
mod tests;
