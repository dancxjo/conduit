//! Admitted native scalar slots beside an unchanged canonical value store.
//!
//! No native result is converted to bytes by this store. A selected Back must
//! explicitly encode a boundary value; generic canonical reads refuse. The
//! scheduler remains responsible for the same logical Cord transactions/Signs.
use crate::{StorageError, ValueRef, ValueStorage};
#[derive(Clone, Copy)]
struct Slot {
    generation: u16,
    references: u16,
    value: u64,
}
const EMPTY: Slot = Slot {
    generation: 0,
    references: 0,
    value: 0,
};
pub struct NativeU64ValueStore<S, const N: usize> {
    canonical: S,
    native: [Slot; N],
    canonical_slots: u16,
    native_byte_capacity: u32,
    native_used: u16,
}
impl<S: ValueStorage, const N: usize> NativeU64ValueStore<S, N> {
    pub fn new(canonical: S, native_byte_capacity: u32) -> Result<Self, StorageError> {
        let count = u16::try_from(N).map_err(|_| StorageError::InvalidBudget)?;
        let physical = u32::from(count)
            .checked_mul(8)
            .ok_or(StorageError::InvalidBudget)?;
        let canonical_slots = canonical.item_capacity();
        if N == 0
            || native_byte_capacity < 8
            || native_byte_capacity > physical
            || canonical_slots.checked_add(count).is_none()
            || canonical
                .byte_capacity()
                .checked_add(native_byte_capacity)
                .is_none()
        {
            return Err(StorageError::InvalidBudget);
        }
        Ok(Self {
            canonical,
            native: [EMPTY; N],
            canonical_slots,
            native_byte_capacity,
            native_used: 0,
        })
    }
    fn native_index(&self, value: ValueRef) -> Option<usize> {
        value
            .slot
            .checked_sub(self.canonical_slots)
            .map(usize::from)
    }
    fn slot(&self, value: ValueRef) -> Result<&Slot, StorageError> {
        let s = self
            .native_index(value)
            .and_then(|i| self.native.get(i))
            .ok_or(StorageError::StaleReference)?;
        if s.references == 0 || s.generation != value.generation || value.byte_len != 8 {
            return Err(StorageError::StaleReference);
        }
        Ok(s)
    }
    fn slot_mut(&mut self, value: ValueRef) -> Result<&mut Slot, StorageError> {
        self.slot(value)?;
        let index = self.native_index(value).unwrap();
        Ok(&mut self.native[index])
    }
    pub fn native_used_items(&self) -> u16 {
        self.native_used
    }
    pub fn canonical(&self) -> &S {
        &self.canonical
    }
}
impl<S: ValueStorage, const N: usize> ValueStorage for NativeU64ValueStore<S, N> {
    fn native_u64_available(&self) -> (u16, u32) {
        (
            self.native
                .iter()
                .filter(|slot| slot.references == 0 && slot.generation != u16::MAX)
                .count() as u16,
            self.native_byte_capacity - u32::from(self.native_used) * 8,
        )
    }
    fn item_capacity(&self) -> u16 {
        self.canonical_slots + N as u16
    }
    fn byte_capacity(&self) -> u32 {
        self.canonical.byte_capacity() + self.native_byte_capacity
    }
    fn used_items(&self) -> u16 {
        self.canonical.used_items() + self.native_used
    }
    fn used_bytes(&self) -> u32 {
        self.canonical.used_bytes() + u32::from(self.native_used) * 8
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        self.canonical.store(bytes)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        if self.native_index(value).is_some() {
            self.slot(value)?;
            return Err(StorageError::RepresentationMismatch);
        }
        self.canonical.get(value)
    }
    fn store_native_u64(&mut self, value: u64) -> Result<ValueRef, StorageError> {
        if (u32::from(self.native_used) + 1) * 8 > self.native_byte_capacity {
            return Err(StorageError::ByteCapacityExceeded);
        }
        let mut retired = false;
        let (index, slot) = self
            .native
            .iter_mut()
            .enumerate()
            .find(|(_, s)| {
                if s.references == 0 && s.generation == u16::MAX {
                    retired = true;
                }
                s.references == 0 && s.generation != u16::MAX
            })
            .ok_or(if retired {
                StorageError::GenerationExhausted
            } else {
                StorageError::ItemCapacityExceeded
            })?;
        slot.generation += 1;
        slot.references = 1;
        slot.value = value;
        self.native_used += 1;
        Ok(ValueRef {
            slot: self.canonical_slots + index as u16,
            generation: slot.generation,
            byte_len: 8,
        })
    }
    fn native_u64(&self, value: ValueRef) -> Result<Option<u64>, StorageError> {
        if self.native_index(value).is_some() {
            return self.slot(value).map(|s| Some(s.value));
        }
        self.canonical.reference_count(value)?;
        Ok(None)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        if self.native_index(value).is_some() {
            return self.slot(value).map(|s| s.references);
        }
        self.canonical.reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        if self.native_index(value).is_some() {
            let s = self.slot_mut(value)?;
            s.references = s
                .references
                .checked_add(1)
                .ok_or(StorageError::ReferenceOverflow)?;
            return Ok(());
        }
        self.canonical.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        if self.native_index(value).is_some() {
            let s = self.slot_mut(value)?;
            s.references -= 1;
            if s.references == 0 {
                self.native_used -= 1;
            }
            return Ok(());
        }
        self.canonical.release(value)
    }
    fn clear(&mut self) {
        self.canonical.clear();
        for s in &mut self.native {
            s.references = 0;
        }
        self.native_used = 0;
    }
}
#[cfg(test)]
mod tests;
