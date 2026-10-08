//! Exact bounded Value custody with checked static initialization.
use super::{StorageError, ValueRef, ValueStorage};

#[derive(Clone, Copy)]
struct FixedValueSlot<const MAX_VALUE_BYTES: usize> {
    generation: u16,
    references: u16,
    len: u32,
    bytes: [u8; MAX_VALUE_BYTES],
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
    slots: [FixedValueSlot<MAX_VALUE_BYTES>; SLOTS],
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
}

impl<const SLOTS: usize, const MAX_VALUE_BYTES: usize> FixedValueStore<SLOTS, MAX_VALUE_BYTES> {
    /// May initialize statically placed storage without a runtime stack copy.
    /// Static placement and exclusive mutable ownership remain the caller's job.
    pub const fn new(byte_capacity: u32) -> Result<Self, StorageError> {
        let physical_bytes = match SLOTS.checked_mul(MAX_VALUE_BYTES) {
            Some(value) if value <= u32::MAX as usize => value as u32,
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

    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        if bytes.len() > MAX_VALUE_BYTES {
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
            .find(|(_, slot)| slot.references == 0)
            .ok_or(StorageError::ItemCapacityExceeded)?;
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation == 0 {
            slot.generation = 1;
        }
        slot.references = 1;
        slot.len = byte_len;
        slot.bytes[..bytes.len()].copy_from_slice(bytes);
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

    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        let slot = self.slot(value)?;
        Ok(&slot.bytes[..usize::try_from(slot.len).map_err(|_| StorageError::StaleReference)?])
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
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.references = 0;
            slot.len = 0;
        }
        self.used_items = 0;
        self.used_bytes = 0;
    }
}
