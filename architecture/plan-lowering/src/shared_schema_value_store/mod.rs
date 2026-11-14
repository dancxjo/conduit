//! Explicit shared physical node buffers behind unchanged logical references.
//! Construction grants no Plan, lifetime proof, admission or execution authority.
use alloc::vec::Vec;
use conduit_core::{
    BorrowedStructuredValueValidator as Validator, ValidatedCanonicalStructuredValue,
};
use conduit_kernel::{CanonicalValueParts, StorageError as E, ValueRef, ValueStorage};
use core::num::NonZeroU16;
mod preparation;
mod storage;
pub use preparation::PreparationError;

struct LogicalSlot {
    maximum_bytes: usize,
    class: u16,
    generation: u16,
    references: u16,
    buffer: Option<NonZeroU16>,
}
struct PhysicalBuffer {
    node: Vec<u8>,
    class: u16,
    owner: Option<NonZeroU16>,
}
/// All original logical identities and generations survive physical reuse.
/// `item_capacity` is their upper bound, not a physical concurrency promise.
/// Before Play, the caller must independently prove the supplied physical stock
/// covers its selected graph and actual prepared owners. Unknown lifetimes must
/// retain dense stock. Every publication still checks actual free physical stock.
pub struct SharedSchemaValueStore<'a, S> {
    canonical: S,
    canonical_slots: u16,
    classes: Vec<Validator<'a>>,
    logical: Vec<LogicalSlot>,
    physical: Vec<PhysicalBuffer>,
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
}
impl<S: ValueStorage> SharedSchemaValueStore<'_, S> {
    pub fn canonical_storage(&self) -> &S {
        &self.canonical
    }
    pub fn reserved_node_storage_bytes(&self) -> usize {
        self.reserved_metadata_bytes() + self.reserved_payload_bytes()
    }
    pub fn reserved_metadata_bytes(&self) -> usize {
        self.classes.capacity() * core::mem::size_of::<Validator<'_>>()
            + self.logical.capacity() * core::mem::size_of::<LogicalSlot>()
            + self.physical.capacity() * core::mem::size_of::<PhysicalBuffer>()
    }
    pub fn reserved_payload_bytes(&self) -> usize {
        self.physical.iter().map(|b| b.node.capacity()).sum()
    }
    /// Physical buffers for this exact schema, independently of logical IDs.
    pub fn physical_item_capacity(&self, schema: &[u8]) -> u16 {
        self.physical
            .iter()
            .filter(|b| self.classes[usize::from(b.class)].type_bytes() == schema)
            .count() as u16
    }
    pub fn free_physical_items(&self, schema: &[u8]) -> u16 {
        self.physical
            .iter()
            .filter(|b| {
                b.owner.is_none() && self.classes[usize::from(b.class)].type_bytes() == schema
            })
            .count() as u16
    }
    fn index(&self, value: ValueRef) -> Option<usize> {
        value
            .slot
            .checked_sub(self.canonical_slots)
            .map(usize::from)
    }
    fn resolve(&self, value: ValueRef) -> Result<(usize, usize), E> {
        let i = self.index(value).ok_or(E::StaleReference)?;
        let slot = self.logical.get(i).ok_or(E::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation {
            return Err(E::StaleReference);
        }
        let j = usize::from(slot.buffer.ok_or(E::StaleReference)?.get() - 1);
        let buffer = self.physical.get(j).ok_or(E::StaleReference)?;
        if buffer.owner.map(NonZeroU16::get) != u16::try_from(i + 1).ok()
            || buffer.class != slot.class
            || self.classes[usize::from(slot.class)]
                .type_bytes()
                .len()
                .checked_add(buffer.node.len())
                .and_then(|n| u32::try_from(n).ok())
                != Some(value.byte_len)
        {
            return Err(E::StaleReference);
        }
        Ok((i, j))
    }
    /// Match the original logical slot order/extent and retirement policy.
    /// Missing physical stock refuses atomically before generation changes.
    pub fn store_structured(
        &mut self,
        value: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<ValueRef, E> {
        let len = value
            .type_bytes()
            .len()
            .checked_add(value.value_node().len())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(E::ValueTooLarge)?;
        let used = self
            .used_bytes
            .checked_add(len)
            .filter(|n| *n <= self.byte_capacity)
            .ok_or(E::ByteCapacityExceeded)?;
        let mut retired = false;
        let mut known = false;
        let i = self
            .logical
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                let fits = self.classes[usize::from(slot.class)].type_bytes() == value.type_bytes()
                    && usize::try_from(len).is_ok_and(|n| n <= slot.maximum_bytes);
                known |= fits;
                retired |= fits && slot.references == 0 && slot.generation == u16::MAX;
                fits && slot.references == 0 && slot.generation != u16::MAX
            })
            .min_by_key(|(_, slot)| slot.maximum_bytes)
            .map(|(i, _)| i)
            .ok_or(if retired {
                E::GenerationExhausted
            } else if known {
                E::ItemCapacityExceeded
            } else {
                E::RepresentationMismatch
            })?;
        let slot = &self.logical[i];
        let class = slot.class;
        self.classes[usize::from(class)]
            .validate_node(value.value_node())
            .map_err(|_| E::RepresentationMismatch)?;
        let j = self
            .physical
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.owner.is_none()
                    && b.class == class
                    && b.node.capacity() >= value.value_node().len()
            })
            .min_by_key(|(_, b)| b.node.capacity())
            .map(|(j, _)| j)
            .ok_or(E::ItemCapacityExceeded)?;
        // All widths were checked in preparation, and every refusal precedes mutation.
        let owner = NonZeroU16::new(u16::try_from(i + 1).map_err(|_| E::InvalidBudget)?)
            .ok_or(E::InvalidBudget)?;
        let physical = NonZeroU16::new(u16::try_from(j + 1).map_err(|_| E::InvalidBudget)?)
            .ok_or(E::InvalidBudget)?;
        let slot = &mut self.logical[i];
        let buffer = &mut self.physical[j];
        slot.generation += 1;
        slot.references = 1;
        slot.buffer = Some(physical);
        buffer.owner = Some(owner);
        buffer.node.clear();
        buffer.node.extend_from_slice(value.value_node());
        self.used_items += 1;
        self.used_bytes = used;
        Ok(ValueRef {
            slot: self.canonical_slots + i as u16,
            generation: slot.generation,
            byte_len: len,
        })
    }
}
#[cfg(test)]
mod tests;
