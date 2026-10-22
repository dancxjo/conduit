//! Preparation-owned observation wrapper. All Play operations still use the
//! hosted store; counters describe actual successful stores, not inferred edges.
use conduit_kernel::{HostedValueStore, StorageError, ValueRef, ValueStorage};
#[derive(Clone, Copy, Default, Debug)]
pub struct StoreMetrics {
    pub canonical_values: u64,
    pub canonical_bytes: u64,
    pub compact_values: u64,
    pub compact_bytes: u64,
    pub compact_logical_bytes: u64,
    pub owned_values: u64,
    pub owned_bytes: u64,
    pub owned_logical_bytes: u64,
}
pub struct ObservedValues {
    inner: HostedValueStore,
    metrics: StoreMetrics,
}
impl ObservedValues {
    pub fn new(items: u16, maximum: u32, bytes: u32) -> Result<Self, StorageError> {
        Ok(Self {
            inner: HostedValueStore::new(items, maximum, bytes)?,
            metrics: StoreMetrics::default(),
        })
    }
    pub fn metrics(&self) -> StoreMetrics {
        self.metrics
    }
}
impl core::ops::Deref for ObservedValues {
    type Target = HostedValueStore;
    fn deref(&self) -> &HostedValueStore {
        &self.inner
    }
}
impl core::ops::DerefMut for ObservedValues {
    fn deref_mut(&mut self) -> &mut HostedValueStore {
        &mut self.inner
    }
}
impl ValueStorage for ObservedValues {
    fn preflight_owned_compact_f32_outputs(
        &self,
        requests: &[(conduit_kernel::scheduler::CompactF32Layout, u16)],
        canonical: &[u32],
    ) -> Result<(), StorageError> {
        self.inner
            .preflight_owned_compact_f32_outputs(requests, canonical)
    }
    fn prepare_owned_compact_f32_outputs(
        &mut self,
        requests: &[(conduit_kernel::scheduler::CompactF32Layout, u16)],
        canonical: &[u32],
    ) -> Result<Vec<conduit_kernel::PreparedCompactF32Output>, StorageError> {
        self.inner
            .prepare_owned_compact_f32_outputs(requests, canonical)
    }
    fn owned_compact_f32_available(
        &self,
        owner: &conduit_kernel::PreparedCompactF32Output,
    ) -> Result<bool, StorageError> {
        self.inner.owned_compact_f32_available(owner)
    }
    fn swap_owned_compact_f32(
        &mut self,
        owner: &conduit_kernel::PreparedCompactF32Output,
        bytes: &mut Vec<u8>,
    ) -> Result<conduit_kernel::PendingCompactF32Publication, StorageError> {
        let size = bytes.len();
        let result = self.inner.swap_owned_compact_f32(owner, bytes)?;
        self.metrics.owned_values += 1;
        self.metrics.owned_bytes += size as u64;
        self.metrics.owned_logical_bytes += u64::from(owner.layout().logical_bytes);
        Ok(result)
    }
    fn commit_owned_compact_f32(
        &mut self,
        owner: &conduit_kernel::PreparedCompactF32Output,
        pending: &mut conduit_kernel::PendingCompactF32Publication,
    ) -> Result<ValueRef, StorageError> {
        self.inner.commit_owned_compact_f32(owner, pending)
    }
    fn restore_owned_compact_f32(
        &mut self,
        owner: &conduit_kernel::PreparedCompactF32Output,
        pending: &mut conduit_kernel::PendingCompactF32Publication,
        bytes: &mut Vec<u8>,
    ) -> Result<(), StorageError> {
        self.inner.restore_owned_compact_f32(owner, pending, bytes)
    }

    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        let value = self.inner.store(bytes)?;
        self.metrics.canonical_values += 1;
        self.metrics.canonical_bytes += bytes.len() as u64;
        Ok(value)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.release(value)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(value)
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
    fn compact_f32_available(&self, logical: u32) -> (u16, u32) {
        self.inner.compact_f32_available(logical)
    }
    fn compact_f32(&self, value: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        self.inner.compact_f32(value)
    }
    fn store_compact_f32(&mut self, bytes: &[u8], logical: u32) -> Result<ValueRef, StorageError> {
        let value = self.inner.store_compact_f32(bytes, logical)?;
        self.metrics.compact_values += 1;
        self.metrics.compact_bytes += bytes.len() as u64;
        self.metrics.compact_logical_bytes += u64::from(logical);
        Ok(value)
    }
}
