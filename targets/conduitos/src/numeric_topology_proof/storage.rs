//! Caller-placed fixed storage. Preparation borrows it; Play never moves it.
use conduit_kernel::{FixedValueStore, StorageError, ValueRef, ValueStorage};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

pub const SLOTS: usize = 1024;
pub const CELL_BYTES: usize = 16384;
pub type Store = FixedValueStore<SLOTS, CELL_BYTES>;

/// One Boot owner may claim this storage once. A failed preparation does not
/// release the claim: accidental second preparation cannot alias live values.
pub struct StaticStorage {
    claimed: AtomicBool,
    store: UnsafeCell<Store>,
}
// SAFETY: compare_exchange grants the only mutable reference once, and neither
// this object nor BorrowedStorage exposes any other access to the UnsafeCell.
unsafe impl Sync for StaticStorage {}
impl StaticStorage {
    pub const fn new() -> Self {
        let store = match Store::new((SLOTS * CELL_BYTES) as u32) {
            Ok(store) => store,
            Err(_) => panic!("fixed numerical proof storage constants"),
        };
        Self {
            claimed: AtomicBool::new(false),
            store: UnsafeCell::new(store),
        }
    }
    pub fn claim(&self) -> Result<BorrowedStorage<'_>, StorageError> {
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| StorageError::InvalidBudget)?;
        // SAFETY: this successful claim is the only access for this lifetime.
        Ok(BorrowedStorage {
            store: unsafe { &mut *self.store.get() },
            peak_items: 0,
            peak_bytes: 0,
        })
    }
}
impl Default for StaticStorage {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BorrowedStorage<'a> {
    store: &'a mut Store,
    peak_items: u16,
    peak_bytes: u32,
}
impl BorrowedStorage<'_> {
    pub fn admit_ingress(
        &mut self,
        source: &impl ValueStorage,
        live: &[ValueRef],
    ) -> Result<(), conduit_kernel::FixedIngressRefusal> {
        conduit_kernel::transfer_fixed_value_ingress(source, self.store, live)?;
        self.observe_peak();
        Ok(())
    }
    pub fn peaks(&self) -> (u16, u32) {
        (self.peak_items, self.peak_bytes)
    }
    fn observe_peak(&mut self) {
        self.peak_items = self.peak_items.max(self.store.used_items());
        self.peak_bytes = self.peak_bytes.max(self.store.used_bytes());
    }
}
impl ValueStorage for BorrowedStorage<'_> {
    fn item_capacity(&self) -> u16 {
        self.store.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.store.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.store.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.store.used_bytes()
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        let value = self.store.store(bytes)?;
        self.observe_peak();
        Ok(value)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        self.store.get(value)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        self.store.reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.store.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.store.release(value)
    }
    fn clear(&mut self) {
        self.store.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static STORAGE: StaticStorage = StaticStorage::new();
    #[test]
    fn static_claim_is_unique_ingress_exact_and_peak_survives_release() {
        let mut input = FixedValueStore::<2, 32>::new(64).unwrap();
        let first = input.store(b"exact fixed ingress").unwrap();
        input.retain(first).unwrap();
        let mut store = STORAGE.claim().unwrap();
        assert!(STORAGE.claim().is_err());
        store.admit_ingress(&input, &[first]).unwrap();
        assert_eq!(store.get(first), input.get(first));
        assert_eq!(store.reference_count(first), Ok(2));
        let next = store.store(b"next").unwrap();
        let peak = store.peaks();
        store.release(next).unwrap();
        assert_eq!(store.peaks(), peak);
        assert_eq!(peak, (2, 23));
        assert_eq!(
            store.store(&[0; CELL_BYTES + 1]),
            Err(StorageError::ValueTooLarge)
        );
        assert_eq!(store.peaks(), peak);
    }
}
