use alloc::vec::Vec;

use super::{StorageError, ValueRef, ValueStorage};
mod compact_f32;
mod native_values;
mod owned_compact_f32;
mod value_admission;

struct HostedValueSlot {
    generation: u16,
    references: u16,
    bytes: Vec<u8>,
}

/// Hosted profile that allocates every slot and byte ceiling before use.
/// Once constructed, `store` never grows a vector.
pub struct HostedValueStore {
    slots: Vec<HostedValueSlot>,
    native_words: Vec<Option<u64>>,
    compact_f32_lengths: Vec<Option<core::num::NonZeroU32>>,
    owned_compact_slots: Vec<Option<owned_compact_f32::OwnedSlot>>,
    owned_compact_identity: Option<alloc::boxed::Box<u8>>,
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
            native_words: Vec::new(),
            compact_f32_lengths: Vec::new(),
            owned_compact_slots: Vec::new(),
            owned_compact_identity: None,
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
    /// from `byte_capacity`, which bounds live logical payload quotas. Compact physical lanes
    /// retain their checked canonical logical bound in this quota.
    pub fn reserved_storage_bytes(&self) -> usize {
        self.slots.capacity() * core::mem::size_of::<HostedValueSlot>()
            + self
                .slots
                .iter()
                .map(|slot| slot.bytes.capacity())
                .sum::<usize>()
            + self.native_words.capacity() * core::mem::size_of::<Option<u64>>()
            + self.compact_f32_lengths.capacity()
                * core::mem::size_of::<Option<core::num::NonZeroU32>>()
            + self.owned_compact_slots.capacity()
                * core::mem::size_of::<Option<owned_compact_f32::OwnedSlot>>()
            + usize::from(self.owned_compact_identity.is_some())
    }

    pub fn allocation_capacities(&self) -> (usize, usize) {
        (
            self.slots.capacity(),
            self.slots
                .iter()
                .map(|slot| slot.bytes.capacity())
                .sum::<usize>()
                + self.native_words.capacity() * core::mem::size_of::<Option<u64>>()
                + self.compact_f32_lengths.capacity()
                    * core::mem::size_of::<Option<core::num::NonZeroU32>>()
                + self.owned_compact_slots.capacity()
                    * core::mem::size_of::<Option<owned_compact_f32::OwnedSlot>>()
                + usize::from(self.owned_compact_identity.is_some()),
        )
    }

    fn slot(&self, value: ValueRef) -> Result<&HostedValueSlot, StorageError> {
        let slot = self
            .slots
            .get(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0
            || slot.generation != value.generation
            || !self.representation_matches(value, slot)
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, value: ValueRef) -> Result<&mut HostedValueSlot, StorageError> {
        self.slot(value)?;
        self.slots
            .get_mut(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)
    }
}

impl ValueStorage for HostedValueStore {
    fn compact_f32_available(&self, logical_bytes: u32) -> (u16, u32) {
        self.available_compact_f32(logical_bytes)
    }
    fn store_compact_f32(
        &mut self,
        bytes: &[u8],
        logical_bytes: u32,
    ) -> Result<ValueRef, StorageError> {
        self.store_compact_lanes(bytes, logical_bytes)
    }
    fn compact_f32(&self, value: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        self.compact_lanes(value)
    }
    fn native_u64_available(&self) -> (u16, u32) {
        self.available_native_words()
    }
    fn store_native_u64(&mut self, value: u64) -> Result<ValueRef, StorageError> {
        self.store_native_word(value)
    }
    fn native_u64(&self, value: ValueRef) -> Result<Option<u64>, StorageError> {
        self.slot(value)?;
        Ok(self
            .native_words
            .get(usize::from(value.slot))
            .copied()
            .flatten())
    }
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
        let value = self.allocate_value_slot(bytes.len())?;
        self.slots[usize::from(value.slot)]
            .bytes
            .extend_from_slice(bytes);
        if let Some(word) = self.native_words.get_mut(usize::from(value.slot)) {
            *word = None;
        }
        self.clear_compact_layout(value.slot);
        Ok(value)
    }

    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        let slot = self.slot(value)?;
        if self.is_compact(value.slot)
            || slot.bytes.len() != usize::try_from(value.byte_len).unwrap_or(usize::MAX)
        {
            return Err(StorageError::RepresentationMismatch);
        }
        Ok(slot.bytes.as_slice())
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
            let len = value.byte_len;
            slot.bytes.clear();
            if let Some(word) = self.native_words.get_mut(usize::from(value.slot)) {
                *word = None;
            }
            self.clear_compact_layout(value.slot);
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
        self.native_words.fill(None);
        self.compact_f32_lengths.fill(None);
        self.used_items = 0;
        self.used_bytes = 0;
        self.free_search_start = 0;
    }
}

pub use HostedValueStore as Store;

#[cfg(test)]
mod tests;
