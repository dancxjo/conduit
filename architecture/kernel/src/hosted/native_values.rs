//! Explicit pre-Play native representation storage beside canonical buffers.
use super::*;
impl HostedValueStore {
    /// Admit the fixed native sidecar during preparation. This is never called
    /// by `store` or a running kernel; the scheduler exposes no mutable store.
    /// Existing canonical resources and reference generations remain unchanged.
    pub fn prepare_native_u64(&mut self) {
        if self.native_words.is_empty() {
            self.native_words.resize(self.slots.len(), None);
        }
    }
    pub(super) fn available_native_words(&self) -> (u16, u32) {
        if self.native_words.is_empty() {
            return (0, 0);
        }
        let items = self
            .slots
            .iter()
            .filter(|s| s.references == 0 && s.generation != u16::MAX && s.bytes.capacity() >= 8)
            .count();
        (items as u16, self.byte_capacity - self.used_bytes)
    }
    pub(super) fn store_native_word(&mut self, word: u64) -> Result<ValueRef, StorageError> {
        if self.native_words.is_empty() {
            return Err(StorageError::RepresentationMismatch);
        }
        let value = self.allocate_value_slot(8)?;
        self.native_words[usize::from(value.slot)] = Some(word);
        Ok(value)
    }
}
#[cfg(test)]
mod tests;
