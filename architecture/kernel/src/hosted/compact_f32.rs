//! Compact physical lanes keep the unchanged canonical logical-byte quota.
//! These bytes are internal finite F32 bits, never a canonical port encoding.
//! Storage alone selects no numerical island or scheduler realization.
use super::{HostedValueSlot, HostedValueStore, StorageError, ValueRef};
use core::num::NonZeroU32;

impl HostedValueStore {
    /// Reserve only representation metadata. Existing preallocated slot buffers
    /// hold the compact payload; no slot or buffer grows during publication.
    pub fn prepare_compact_f32(&mut self) {
        if self.compact_f32_lengths.is_empty() {
            self.compact_f32_lengths.resize(self.slots.len(), None);
        }
    }
    pub(super) fn is_compact(&self, slot: u16) -> bool {
        self.compact_f32_lengths
            .get(usize::from(slot))
            .is_some_and(Option::is_some)
    }
    pub(super) fn clear_compact_layout(&mut self, slot: u16) {
        if let Some(layout) = self.compact_f32_lengths.get_mut(usize::from(slot)) {
            *layout = None;
        }
    }
    pub(super) fn representation_matches(&self, value: ValueRef, slot: &HostedValueSlot) -> bool {
        let word = self
            .native_words
            .get(usize::from(value.slot))
            .is_some_and(Option::is_some);
        let compact = self
            .compact_f32_lengths
            .get(usize::from(value.slot))
            .copied()
            .flatten();
        match (word, compact) {
            (true, None) => value.byte_len == 8 && slot.bytes.is_empty(),
            (false, Some(logical)) => {
                logical.get() == value.byte_len
                    && !slot.bytes.is_empty()
                    && slot.bytes.len().is_multiple_of(4)
                    && usize::try_from(logical.get()).is_ok_and(|n| slot.bytes.len() <= n)
            }
            (false, None) => {
                slot.bytes.len() == usize::try_from(value.byte_len).unwrap_or(usize::MAX)
            }
            (true, Some(_)) => false,
        }
    }
    pub(super) fn available_compact_f32(&self, logical_bytes: u32) -> (u16, u32) {
        if self.compact_f32_lengths.is_empty() || logical_bytes < 4 {
            return (0, 0);
        }
        let Ok(logical_size) = usize::try_from(logical_bytes) else {
            return (0, 0);
        };
        let count = self
            .slots
            .iter()
            .filter(|slot| {
                slot.references == 0
                    && slot.generation != u16::MAX
                    && slot.bytes.capacity() >= logical_size
            })
            .count();
        (count as u16, self.byte_capacity - self.used_bytes)
    }
    pub(super) fn store_compact_lanes(
        &mut self,
        bytes: &[u8],
        logical_bytes: u32,
    ) -> Result<ValueRef, StorageError> {
        if self.compact_f32_lengths.is_empty() {
            return Err(StorageError::RepresentationMismatch);
        }
        let logical = NonZeroU32::new(logical_bytes).ok_or(StorageError::InvalidBudget)?;
        let logical_size =
            usize::try_from(logical_bytes).map_err(|_| StorageError::InvalidBudget)?;
        if bytes.len() > logical_size {
            return Err(StorageError::InvalidBudget);
        }
        let (lanes, remainder) = bytes.as_chunks::<4>();
        if lanes.is_empty()
            || !remainder.is_empty()
            || lanes
                .iter()
                .any(|lane| !f32::from_le_bytes(*lane).is_finite())
        {
            return Err(StorageError::RepresentationMismatch);
        }
        let value = self.allocate_value_slot(logical_size)?;
        let index = usize::from(value.slot);
        self.slots[index].bytes.extend_from_slice(bytes);
        if let Some(word) = self.native_words.get_mut(index) {
            *word = None;
        }
        self.compact_f32_lengths[index] = Some(logical);
        Ok(value)
    }
    pub(super) fn compact_lanes(&self, value: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        let slot = self.slot(value)?;
        Ok(self.is_compact(value.slot).then_some(slot.bytes.as_slice()))
    }
}
#[cfg(test)]
mod tests;
