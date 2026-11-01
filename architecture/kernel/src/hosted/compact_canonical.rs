//! Explicit compact slots retain unchanged canonical logical quotas.
use super::*;
use crate::CompactCanonicalLayout;
use alloc::boxed::Box;
use core::num::NonZeroU32;

pub(super) struct Storage {
    pub(super) layout: CompactCanonicalLayout,
    start: usize,
    canonical_limit: usize,
    lengths: Vec<Option<NonZeroU32>>,
}
impl HostedValueStore {
    /// Exact retained physical backing; excludes the store object, allocator
    /// overhead. Construction streams the slot-capacity profile without copying it.
    pub fn compact_canonical_storage_bytes(
        canonical: &[u32],
        compact_items: u16,
        layout: CompactCanonicalLayout,
    ) -> Result<usize, StorageError> {
        if compact_items == 0 {
            return Err(StorageError::InvalidBudget);
        }
        let mut bytes =
            Self::canonical_storage_bytes(canonical.iter().copied().chain(core::iter::repeat_n(
                u32::from(layout.physical_bytes()),
                usize::from(compact_items),
            )))?;
        bytes = bytes
            .checked_add(core::mem::size_of::<Storage>())
            .and_then(|n| {
                n.checked_add(
                    usize::from(compact_items) * core::mem::size_of::<Option<NonZeroU32>>(),
                )
            })
            .ok_or(StorageError::InvalidBudget)?;
        Ok(bytes)
    }
    /// Shared slots may hold ordinary canonical bytes or the selected compact
    /// representation. This preserves small-value eligibility under interleaved
    /// ownership; no class-specific live reservation can strand a free slot.
    pub fn new_with_shared_compact_canonical_slots(
        capacities: &[u32],
        layout: CompactCanonicalLayout,
        logical_byte_capacity: u32,
    ) -> Result<Self, StorageError> {
        Self::new_compact_profile(capacities, 0, layout, logical_byte_capacity, true)
    }
    pub fn shared_compact_canonical_storage_bytes(
        capacities: &[u32],
    ) -> Result<usize, StorageError> {
        Self::canonical_storage_bytes(capacities.iter().copied())?
            .checked_add(core::mem::size_of::<Storage>())
            .and_then(|n| {
                n.checked_add(capacities.len() * core::mem::size_of::<Option<NonZeroU32>>())
            })
            .ok_or(StorageError::InvalidBudget)
    }
    /// Preparation only. These physical slots are never available to generic
    /// canonical writes. No consumer binding or execution authority is created.
    pub fn new_with_compact_canonical_slots(
        canonical: &[u32],
        compact_items: u16,
        layout: CompactCanonicalLayout,
        logical_byte_capacity: u32,
    ) -> Result<Self, StorageError> {
        Self::new_compact_profile(
            canonical,
            compact_items,
            layout,
            logical_byte_capacity,
            false,
        )
    }
    fn new_compact_profile(
        canonical: &[u32],
        compact_items: u16,
        layout: CompactCanonicalLayout,
        logical_byte_capacity: u32,
        shared: bool,
    ) -> Result<Self, StorageError> {
        if compact_items == 0 && !shared {
            return Err(StorageError::InvalidBudget);
        }
        let count = canonical
            .len()
            .checked_add(usize::from(compact_items))
            .ok_or(StorageError::InvalidBudget)?;
        if count == 0 || count > usize::from(u16::MAX) {
            return Err(StorageError::InvalidBudget);
        }
        let canonical_bytes = canonical
            .iter()
            .try_fold(0u32, |n, b| n.checked_add(*b))
            .ok_or(StorageError::InvalidBudget)?;
        let maximum = if shared {
            canonical
                .iter()
                .try_fold(0u32, |n, capacity| {
                    n.checked_add(if *capacity >= u32::from(layout.physical_bytes()) {
                        (*capacity).max(layout.logical_bytes())
                    } else {
                        *capacity
                    })
                })
                .ok_or(StorageError::InvalidBudget)?
        } else {
            canonical_bytes
                .checked_add(
                    u32::from(compact_items)
                        .checked_mul(layout.logical_bytes())
                        .ok_or(StorageError::InvalidBudget)?,
                )
                .ok_or(StorageError::InvalidBudget)?
        };
        if logical_byte_capacity == 0 || logical_byte_capacity > maximum {
            return Err(StorageError::InvalidBudget);
        }
        if canonical.contains(&0) {
            return Err(StorageError::InvalidBudget);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|_| StorageError::ByteCapacityExceeded)?;
        for capacity in canonical.iter().copied().chain(core::iter::repeat_n(
            u32::from(layout.physical_bytes()),
            usize::from(compact_items),
        )) {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(
                    usize::try_from(capacity).map_err(|_| StorageError::InvalidBudget)?,
                )
                .map_err(|_| StorageError::ByteCapacityExceeded)?;
            slots.push(HostedValueSlot {
                generation: 0,
                references: 0,
                bytes,
            });
        }
        let compact_count = if shared {
            count
        } else {
            usize::from(compact_items)
        };
        let mut lengths = Vec::new();
        lengths
            .try_reserve_exact(compact_count)
            .map_err(|_| StorageError::ByteCapacityExceeded)?;
        lengths.resize(compact_count, None);
        let storage = Storage {
            layout,
            start: if shared { 0 } else { canonical.len() },
            canonical_limit: canonical.len(),
            lengths,
        };
        let memory = core::alloc::Layout::new::<Storage>();
        // SAFETY: exact nonzero layout, converted to a uniquely owned Box only
        // after a successful allocation; null leaves all fresh buffers owned.
        let pointer =
            unsafe { alloc::alloc::alloc(memory) }.cast::<core::mem::MaybeUninit<Storage>>();
        if pointer.is_null() {
            return Err(StorageError::ByteCapacityExceeded);
        }
        let storage = Box::write(unsafe { Box::from_raw(pointer) }, storage);
        Ok(Self {
            slots,
            native_words: Vec::new(),
            compact_canonical: Some(storage),
            compact_f32_lengths: Vec::new(),
            owned_compact_slots: Vec::new(),
            owned_compact_identity: None,
            maximum_value_bytes: canonical.iter().copied().max().unwrap_or(0) as usize,
            byte_capacity: logical_byte_capacity,
            used_items: 0,
            used_bytes: 0,
            uniform_slots: false,
            free_search_start: 0,
            has_retired_slots: false,
            #[cfg(test)]
            slot_probes: 0,
        })
    }
    pub(super) fn canonical_slot_limit(&self) -> usize {
        self.compact_canonical
            .as_ref()
            .map_or(self.slots.len(), |s| s.canonical_limit)
    }
    pub(super) fn is_canonical_compact_slot(&self, slot: usize) -> bool {
        self.compact_canonical_length(slot).is_some()
    }
    pub(super) fn compact_canonical_length(&self, slot: usize) -> Option<u32> {
        let storage = self.compact_canonical.as_ref()?;
        storage
            .lengths
            .get(slot.checked_sub(storage.start)?)?
            .map(NonZeroU32::get)
    }
    pub(super) fn clear_compact_canonical(&mut self, slot: u16) {
        if let Some(s) = self.compact_canonical.as_mut() {
            if let Some(n) = usize::from(slot)
                .checked_sub(s.start)
                .and_then(|i| s.lengths.get_mut(i))
            {
                *n = None;
            }
        }
    }
    pub(super) fn clear_all_compact_canonical(&mut self) {
        if let Some(s) = self.compact_canonical.as_mut() {
            s.lengths.fill(None);
        }
    }
    pub(super) fn compact_canonical_backing_bytes(&self) -> usize {
        self.compact_canonical.as_ref().map_or(0, |s| {
            core::mem::size_of::<Storage>()
                + s.lengths.capacity() * core::mem::size_of::<Option<NonZeroU32>>()
        })
    }
    pub(super) fn available_compact_canonical(&self, layout: CompactCanonicalLayout) -> (u16, u32) {
        let Some(s) = self
            .compact_canonical
            .as_ref()
            .filter(|s| s.layout == layout)
        else {
            return (0, 0);
        };
        let count = self.slots[s.start..]
            .iter()
            .filter(|slot| {
                slot.references == 0
                    && slot.generation != u32::MAX
                    && slot.bytes.capacity() >= usize::from(layout.physical_bytes())
            })
            .count();
        (count as u16, self.byte_capacity - self.used_bytes)
    }
    pub(super) fn store_canonical_compact(
        &mut self,
        layout: CompactCanonicalLayout,
        bytes: &[u8],
        logical: u32,
    ) -> Result<ValueRef, StorageError> {
        let s = self
            .compact_canonical
            .as_ref()
            .filter(|s| s.layout == layout)
            .ok_or(StorageError::RepresentationMismatch)?;
        let logical = NonZeroU32::new(logical).ok_or(StorageError::InvalidBudget)?;
        if bytes.is_empty()
            || bytes.len() > usize::from(layout.physical_bytes())
            || logical.get() > layout.logical_bytes()
            || u32::try_from(bytes.len()).map_or(true, |n| n > logical.get())
        {
            return Err(StorageError::ValueTooLarge);
        }
        let next_used = self
            .used_bytes
            .checked_add(logical.get())
            .filter(|n| *n <= self.byte_capacity)
            .ok_or(StorageError::ByteCapacityExceeded)?;
        let start = s.start;
        let retired = self.slots[start..].iter().any(|slot| {
            slot.references == 0
                && slot.generation == u32::MAX
                && slot.bytes.capacity() >= usize::from(layout.physical_bytes())
        });
        let (offset, slot) = self.slots[start..]
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| {
                slot.references == 0
                    && slot.generation != u32::MAX
                    && slot.bytes.capacity() >= usize::from(layout.physical_bytes())
            })
            .ok_or(if retired {
                StorageError::GenerationExhausted
            } else {
                StorageError::ItemCapacityExceeded
            })?;
        // Every bound and ownership preflight precedes publication. Buffers were
        // reserved during preparation and cannot grow here.
        slot.bytes.clear();
        slot.bytes.extend_from_slice(bytes);
        slot.generation += 1;
        slot.references = 1;
        self.compact_canonical
            .as_mut()
            .expect("checked compact owner")
            .lengths[offset] = Some(logical);
        self.used_items += 1;
        self.used_bytes = next_used;
        Ok(ValueRef {
            slot: (start + offset) as u16,
            generation: slot.generation,
            byte_len: logical.get(),
        })
    }
    pub(super) fn read_compact_canonical(
        &self,
        value: ValueRef,
        layout: CompactCanonicalLayout,
    ) -> Result<Option<&[u8]>, StorageError> {
        let slot = self.slot(value)?;
        if !self.is_canonical_compact_slot(usize::from(value.slot)) {
            return Ok(None);
        }
        if self
            .compact_canonical
            .as_ref()
            .is_none_or(|s| s.layout != layout)
        {
            return Err(StorageError::RepresentationMismatch);
        }
        Ok(Some(&slot.bytes))
    }
}

#[cfg(test)]
mod tests;
