//! Typed output pools cannot be reused as canonical or copied-output slots.
use super::*;
use crate::{owned_compact_output::*, scheduler::CompactF32Layout};
use alloc::{boxed::Box, vec};
use core::num::NonZeroU32;

pub(super) struct OwnedSlot {
    owner: usize,
    layout: CompactF32Layout,
    pending: bool,
    pending_identity: usize,
    identity: Option<Box<u8>>,
}
impl HostedValueStore {
    pub(super) fn is_owned_compact(&self, slot: usize) -> bool {
        self.owned_compact_slots
            .get(slot)
            .is_some_and(Option::is_some)
    }
    pub(super) fn clear_owned_pending(&mut self, index: usize) {
        if let Some(Some(slot)) = self.owned_compact_slots.get_mut(index) {
            slot.pending = false;
            slot.pending_identity = 0;
            if slot.identity.is_none() {
                self.slots[index].generation = u16::MAX;
                self.has_retired_slots = true;
            }
        }
    }
    pub(super) fn reset_owned_pending(&mut self) {
        for index in 0..self.owned_compact_slots.len() {
            self.clear_owned_pending(index);
        }
    }
    pub(super) fn owned_compact_backing_bytes(&self) -> usize {
        self.owned_compact_slots.capacity() * core::mem::size_of::<Option<OwnedSlot>>()
            + usize::from(self.owned_compact_identity.is_some())
            + self
                .owned_compact_slots
                .iter()
                .flatten()
                .filter(|slot| slot.identity.is_some() || slot.pending)
                .count()
    }
    fn owned_store_identity(&self) -> Option<usize> {
        self.owned_compact_identity
            .as_deref()
            .map(|v| core::ptr::from_ref(v) as usize)
    }
    fn owned_selection(
        &self,
        requests: &[(CompactF32Layout, u16)],
        canonical_reservations: &[u32],
    ) -> Result<Vec<Vec<u16>>, StorageError> {
        if requests.is_empty() || requests.len() > 32 || canonical_reservations.contains(&0) {
            return Err(StorageError::InvalidBudget);
        }
        let count = requests
            .iter()
            .try_fold(0usize, |n, (_, slots)| n.checked_add(usize::from(*slots)))
            .ok_or(StorageError::InvalidBudget)?;
        if count > self.slots.len() {
            return Err(StorageError::ItemCapacityExceeded);
        }
        let mut available: Vec<_> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(index, slot)| {
                slot.references == 0
                    && slot.generation != u16::MAX
                    && !self.is_owned_compact(*index)
            })
            .map(|(index, _)| index)
            .collect();
        available.sort_unstable_by_key(|index| (self.slots[*index].bytes.capacity(), *index));
        let mut selected: Vec<Vec<u16>> = Vec::with_capacity(requests.len());
        for (layout, count) in requests {
            let physical = layout
                .physical_bytes()
                .map_err(|_| StorageError::InvalidBudget)?;
            if *count == 0 || physical > layout.logical_bytes {
                return Err(StorageError::InvalidBudget);
            }
            let mut group = Vec::with_capacity(usize::from(*count));
            for _ in 0..*count {
                let position = available.partition_point(|index| {
                    self.slots[*index].bytes.capacity() < layout.logical_bytes as usize
                });
                if position == available.len() {
                    return Err(StorageError::ItemCapacityExceeded);
                }
                let index = available.remove(position);
                group.push(index as u16);
            }
            selected.push(group);
        }
        // Reserve the remaining generic slots by exact capacity. Existing live
        // values and earlier typed pools cannot satisfy these obligations.
        for bytes in canonical_reservations {
            let position = available
                .partition_point(|index| self.slots[*index].bytes.capacity() < *bytes as usize);
            if position == available.len() {
                return Err(StorageError::ItemCapacityExceeded);
            }
            available.remove(position);
        }
        Ok(selected)
    }
    /// Read-only preflight; publication of the batch is all-or-nothing.
    /// Each canonical reservation requires one additional free generic slot
    /// of at least the named capacity after conversion. This is a preparation
    /// check; the caller still admits shared live-byte quota for all traffic.
    pub fn preflight_owned_compact_f32_outputs(
        &self,
        requests: &[(CompactF32Layout, u16)],
        canonical_reservations: &[u32],
    ) -> Result<(), StorageError> {
        self.owned_selection(requests, canonical_reservations)
            .map(|_| ())
    }
    /// Convert free admitted slots before Play, retaining their logical quota.
    /// Physical buffers become exact typed lane buffers; generic stores skip them.
    pub fn prepare_owned_compact_f32_outputs(
        &mut self,
        requests: &[(CompactF32Layout, u16)],
        canonical_reservations: &[u32],
    ) -> Result<Vec<PreparedCompactF32Output>, StorageError> {
        let selected = self.owned_selection(requests, canonical_reservations)?;
        // Allocate the entire finite candidate batch before changing store truth.
        let mut buffers: Vec<Vec<Vec<u8>>> = requests
            .iter()
            .map(|(layout, count)| {
                (0..*count)
                    .map(|_| vec![0; layout.physical_bytes().expect("preflight layout") as usize])
                    .collect()
            })
            .collect();
        let identities: Vec<Vec<Box<u8>>> = requests
            .iter()
            .map(|(_, count)| (0..*count).map(|_| Box::new(0)).collect())
            .collect();
        let identity = self
            .owned_compact_identity
            .take()
            .unwrap_or_else(|| Box::new(0));
        let store = core::ptr::from_ref(identity.as_ref()) as usize;
        let outputs: Vec<_> = selected
            .into_iter()
            .zip(requests)
            .map(|(slots, (layout, _))| PreparedCompactF32Output {
                store,
                identity: Box::new(0),
                slots,
                layout: *layout,
            })
            .collect();
        self.prepare_compact_f32();
        if self.owned_compact_slots.is_empty() {
            self.owned_compact_slots
                .resize_with(self.slots.len(), || None);
        }
        for ((output, buffers), identities) in outputs.iter().zip(&mut buffers).zip(identities) {
            for ((index, bytes), identity) in output.slots.iter().zip(buffers).zip(identities) {
                let index = usize::from(*index);
                self.slots[index].bytes = core::mem::take(bytes);
                self.owned_compact_slots[index] = Some(OwnedSlot {
                    owner: output.identity(),
                    layout: output.layout,
                    pending: false,
                    pending_identity: 0,
                    identity: Some(identity),
                });
                self.clear_compact_layout(index as u16);
                if let Some(word) = self.native_words.get_mut(index) {
                    *word = None;
                }
            }
        }
        self.owned_compact_identity = Some(identity);
        self.uniform_slots = false;
        Ok(outputs)
    }
    fn owned_slot(
        &self,
        owner: &PreparedCompactF32Output,
        index: u16,
    ) -> Result<&OwnedSlot, StorageError> {
        if self.owned_store_identity() != Some(owner.store) || !owner.slots.contains(&index) {
            return Err(StorageError::StaleReference);
        }
        self.owned_compact_slots
            .get(usize::from(index))
            .and_then(Option::as_ref)
            .filter(|slot| slot.owner == owner.identity() && slot.layout == owner.layout)
            .ok_or(StorageError::StaleReference)
    }
    pub fn owned_compact_f32_available(
        &self,
        owner: &PreparedCompactF32Output,
    ) -> Result<bool, StorageError> {
        for index in &owner.slots {
            self.owned_slot(owner, *index)?;
            let slot = &self.slots[usize::from(*index)];
            if slot.references == 0
                && slot.generation != u16::MAX
                && self
                    .used_bytes
                    .checked_add(owner.layout.logical_bytes)
                    .is_some_and(|n| n <= self.byte_capacity)
            {
                return Ok(true);
            }
        }
        if owner
            .slots
            .iter()
            .all(|index| self.slots[usize::from(*index)].generation == u16::MAX)
        {
            return Err(StorageError::GenerationExhausted);
        }
        Ok(false)
    }
    /// Transfer without a payload copy. The producer receives another exact,
    /// empty, already-admitted lane buffer and cannot alias a live input.
    pub fn swap_owned_compact_f32(
        &mut self,
        owner: &PreparedCompactF32Output,
        bytes: &mut Vec<u8>,
    ) -> Result<PendingCompactF32Publication, StorageError> {
        let physical = owner
            .layout
            .physical_bytes()
            .map_err(|_| StorageError::InvalidBudget)? as usize;
        if bytes.len() != physical
            || bytes.capacity() != physical
            || bytes
                .as_chunks::<4>()
                .0
                .iter()
                .any(|v| !f32::from_le_bytes(*v).is_finite())
        {
            return Err(StorageError::RepresentationMismatch);
        }
        for index in &owner.slots {
            self.owned_slot(owner, *index)?;
        }
        let logical = owner.layout.logical_bytes;
        let used = self
            .used_bytes
            .checked_add(logical)
            .filter(|n| *n <= self.byte_capacity)
            .ok_or(StorageError::ByteCapacityExceeded)?;
        let index = owner
            .slots
            .iter()
            .copied()
            .find(|index| {
                let slot = &self.slots[usize::from(*index)];
                slot.references == 0 && slot.generation != u16::MAX
            })
            .ok_or(
                if owner
                    .slots
                    .iter()
                    .any(|i| self.slots[usize::from(*i)].generation == u16::MAX)
                {
                    StorageError::GenerationExhausted
                } else {
                    StorageError::ItemCapacityExceeded
                },
            )?;
        let slot = &mut self.slots[usize::from(index)];
        core::mem::swap(bytes, &mut slot.bytes);
        bytes.clear();
        slot.generation += 1;
        slot.references = 1;
        self.used_bytes = used;
        self.used_items += 1;
        self.compact_f32_lengths[usize::from(index)] = NonZeroU32::new(logical);
        let metadata = self.owned_compact_slots[usize::from(index)]
            .as_mut()
            .unwrap();
        let identity = metadata
            .identity
            .take()
            .expect("free pool slot retains its prepared receipt");
        metadata.pending_identity = core::ptr::from_ref(identity.as_ref()) as usize;
        metadata.pending = true;
        Ok(PendingCompactF32Publication {
            identity: Some(identity),
            value: Some(ValueRef {
                slot: index,
                generation: slot.generation,
                byte_len: logical,
            }),
        })
    }
    pub fn commit_owned_compact_f32(
        &mut self,
        owner: &PreparedCompactF32Output,
        publication: &mut PendingCompactF32Publication,
    ) -> Result<ValueRef, StorageError> {
        let value = publication.value()?;
        let metadata = self.owned_slot(owner, value.slot)?;
        self.slot(value)?;
        if !metadata.pending || publication.identity() != Some(metadata.pending_identity) {
            return Err(StorageError::StaleReference);
        }
        let metadata = self.owned_compact_slots[usize::from(value.slot)]
            .as_mut()
            .unwrap();
        metadata.identity = publication.identity.take();
        metadata.pending_identity = 0;
        metadata.pending = false;
        publication.value = None;
        Ok(value)
    }
    pub fn restore_owned_compact_f32(
        &mut self,
        owner: &PreparedCompactF32Output,
        publication: &mut PendingCompactF32Publication,
        bytes: &mut Vec<u8>,
    ) -> Result<(), StorageError> {
        let value = publication.value()?;
        let metadata = self.owned_slot(owner, value.slot)?;
        let slot = self.slot(value)?;
        if !metadata.pending
            || publication.identity() != Some(metadata.pending_identity)
            || slot.references != 1
            || !bytes.is_empty()
            || bytes.capacity()
                != owner
                    .layout
                    .physical_bytes()
                    .map_err(|_| StorageError::InvalidBudget)? as usize
        {
            return Err(StorageError::StaleReference);
        }
        let slot = &mut self.slots[usize::from(value.slot)];
        core::mem::swap(bytes, &mut slot.bytes);
        slot.references = 0;
        slot.bytes.clear();
        self.used_items -= 1;
        self.used_bytes -= value.byte_len;
        self.clear_compact_layout(value.slot);
        self.has_retired_slots |= value.generation == u16::MAX;
        publication.value = None;
        let metadata = self.owned_compact_slots[usize::from(value.slot)]
            .as_mut()
            .unwrap();
        metadata.identity = publication.identity.take();
        metadata.pending_identity = 0;
        metadata.pending = false;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
