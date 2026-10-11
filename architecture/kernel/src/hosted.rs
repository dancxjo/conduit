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
        Ok(Self {
            slots,
            maximum_value_bytes: maximum_value_bytes as usize,
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
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
        let (slot_index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .filter(|(_, slot)| slot.references == 0 && slot.bytes.capacity() >= bytes.len())
            .min_by_key(|(_, slot)| slot.bytes.capacity())
            .ok_or(StorageError::ItemCapacityExceeded)?;
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation == 0 {
            slot.generation = 1;
        }
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
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.references = 0;
            slot.bytes.clear();
        }
        self.used_items = 0;
        self.used_bytes = 0;
    }
}

pub use HostedValueStore as Store;
