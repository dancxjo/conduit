//! Opaque ownership of pre-Play compact output slots and a pending publication.
use crate::{scheduler::CompactF32Layout, ValueRef};
use alloc::{boxed::Box, vec::Vec};

/// One kernel-owned producer pool. No caller can construct or clone its identity.
/// The store anchor and this live allocation jointly prevent address reuse from
/// substituting a foreign owner. Pool membership also survives owner replacement.
#[derive(Debug)]
pub struct PreparedCompactF32Output {
    pub(crate) store: usize,
    pub(crate) identity: Box<u8>,
    pub(crate) slots: Vec<u16>,
    pub(crate) layout: CompactF32Layout,
}
impl PreparedCompactF32Output {
    pub fn layout(&self) -> CompactF32Layout {
        self.layout
    }
    /// Heap backing retained by the opaque producer-pool handle.
    pub fn reserved_storage_bytes(&self) -> usize {
        self.slots.capacity() * core::mem::size_of::<u16>() + core::mem::size_of::<u8>()
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
    pub(crate) fn identity(&self) -> usize {
        core::ptr::from_ref(self.identity.as_ref()) as usize
    }
}

/// An unpublished buffer transfer. Commit consumes this handle; rollback returns
/// the exact producer buffer and releases the logical quota without copying it.
#[must_use = "commit or restore a pending compact output publication"]
#[derive(Debug)]
pub struct PendingCompactF32Publication {
    pub(crate) value: Option<ValueRef>,
}
impl PendingCompactF32Publication {
    pub fn value(&self) -> Result<ValueRef, crate::StorageError> {
        self.value.ok_or(crate::StorageError::StaleReference)
    }
}
