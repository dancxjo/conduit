//! Optional preallocated bytes; transient event order supplies collision-free
//! ring slots. Every retained ValueRouted event lies within the last N events:
//! otherwise it and its N newer events would exceed the retained-event ceiling.
//! Pinned terminal/remote events consume slots and cannot extend this window.
use super::{ContentSignBinding, SignError};

/// Fixed byte payload only; excludes container and allocator overhead. Refuses
/// ceilings that exceed either the address space or the u32 budget vocabulary.
pub fn content_value_storage_bytes(items: u16, maximum_value_bytes: u32) -> Option<u32> {
    usize::from(items)
        .checked_mul(usize::try_from(maximum_value_bytes).ok()?)
        .and_then(|n| u32::try_from(n).ok())
}
pub(super) struct ValueStorage<V> {
    bytes: V,
    maximum: Option<u32>,
}
impl ValueStorage<[u8; 0]> {
    pub(super) const fn disabled() -> Self {
        Self {
            bytes: [],
            maximum: None,
        }
    }
}
impl<V: AsRef<[u8]> + AsMut<[u8]>> ValueStorage<V> {
    pub(super) fn new(bytes: V, slots: u16, maximum: u32) -> Result<Self, SignError> {
        let size = content_value_storage_bytes(slots, maximum).ok_or(SignError::InvalidBudget)?;
        if slots == 0 || bytes.as_ref().len() != size as usize {
            return Err(SignError::InvalidBudget);
        }
        Ok(Self {
            bytes,
            maximum: Some(maximum),
        })
    }
    pub(super) fn maximum(&self) -> Option<u32> {
        self.maximum
    }
    pub(super) fn capacity_bytes(&self) -> u32 {
        self.bytes.as_ref().len() as u32
    }
    pub(super) fn admit_extent(&self, length: u32) -> Result<(), SignError> {
        if self.maximum.is_some_and(|max| length > max) {
            return Err(SignError::InvalidBudget);
        }
        Ok(())
    }
    pub(super) fn record(&mut self, sequence: u32, slots: u16, bytes: &[u8]) {
        let Some(maximum) = self.maximum else {
            return;
        };
        let start = (sequence % u32::from(slots)) as usize * maximum as usize;
        self.bytes.as_mut()[start..start + bytes.len()].copy_from_slice(bytes);
    }
    pub(super) fn get(&self, binding: ContentSignBinding, slots: u16) -> Option<&[u8]> {
        let maximum = self.maximum?;
        let start = (binding.event_sequence % u32::from(slots)) as usize * maximum as usize;
        self.bytes
            .as_ref()
            .get(start..start + binding.byte_length as usize)
    }
}
