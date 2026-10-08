//! Hosted preparation adapter for caller-owned fixed Play storage.
//! Preparation may allocate; no storage allocation occurs during scheduler Play.
use super::*;
use conduit_kernel::{FixedValueStore, StorageError};
#[path = "../../../../../proof/fargan/fixed_storage_ingress.rs"]
mod ingress;
#[cfg(test)]
#[path = "../../../../../proof/fargan/fixed_storage_ingress_tests.rs"]
mod ingress_tests;

pub(super) enum RuntimeValueStorage {
    Hosted(HostedValueStore),
    Fixed {
        storage: Box<FixedValueStore<1024, 16384>>,
        maximum_items: u16,
    },
}
pub(super) fn prepare(
    source: HostedValueStore,
    fixtures: &BTreeMap<String, Vec<ValueRef>>,
    fixed: bool,
) -> RuntimeValueStorage {
    if !fixed {
        return RuntimeValueStorage::Hosted(source);
    }
    let mut references: Vec<_> = fixtures.values().flatten().copied().collect();
    references.sort_by_key(|reference| reference.slot);
    references.dedup();
    let maximum_live = references
        .iter()
        .map(|reference| source.get(*reference).unwrap().len())
        .max()
        .unwrap_or(0);
    assert!(
        maximum_live <= 16384,
        "exact fixed-cell admission; full Native custody stays session-owned"
    );
    eprintln!("fixed ingress: {} live cells, maximum{}B, retained{}B, selected item budget{}/byte budget{}B", references.len(), maximum_live, source.used_bytes(), source.item_capacity(), source.byte_capacity());
    // The hosted proof gives construction ample stack; a target must place its
    // fixed storage in explicitly admitted static memory instead of this Box.
    let maximum_items = source.item_capacity();
    let maximum_bytes = source.byte_capacity();
    let mut destination = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || Box::new(FixedValueStore::<1024, 16384>::new(maximum_bytes).unwrap()))
        .unwrap()
        .join()
        .unwrap();
    ingress::transfer(&source, &mut destination, &references).unwrap();
    RuntimeValueStorage::Fixed {
        storage: destination,
        maximum_items,
    }
}
macro_rules! delegate {
    ($self:ident, $method:ident $(, $argument:expr)*) => {
        match $self {
            RuntimeValueStorage::Hosted(storage) => storage.$method($($argument),*),
            RuntimeValueStorage::Fixed { storage, .. } => storage.$method($($argument),*),
        }
    };
}
impl ValueStorage for RuntimeValueStorage {
    fn item_capacity(&self) -> u16 {
        match self {
            Self::Hosted(storage) => storage.item_capacity(),
            Self::Fixed { maximum_items, .. } => *maximum_items,
        }
    }
    fn byte_capacity(&self) -> u32 {
        delegate!(self, byte_capacity)
    }
    fn used_items(&self) -> u16 {
        delegate!(self, used_items)
    }
    fn used_bytes(&self) -> u32 {
        delegate!(self, used_bytes)
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        if self.used_items() >= self.item_capacity() {
            return Err(StorageError::ItemCapacityExceeded);
        }
        delegate!(self, store, bytes)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        delegate!(self, get, value)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        delegate!(self, reference_count, value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        delegate!(self, retain, value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        delegate!(self, release, value)
    }
    fn clear(&mut self) {
        delegate!(self, clear)
    }
}
