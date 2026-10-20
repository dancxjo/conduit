use conduit_kernel::{HostedValueStore, StorageError, ValueRef, ValueStorage};
pub(super) struct Counted {
    pub(super) inner: HostedValueStore,
    pub canonical: u64,
    pub compact: u64,
    pub physical: u64,
}
impl ValueStorage for Counted {
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
    fn store(&mut self, b: &[u8]) -> Result<ValueRef, StorageError> {
        let v = self.inner.store(b)?;
        self.canonical += 1;
        Ok(v)
    }
    fn get(&self, v: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(v)
    }
    fn retain(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(v)
    }
    fn release(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.release(v)
    }
    fn reference_count(&self, v: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(v)
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
    fn compact_f32_available(&self, n: u32) -> (u16, u32) {
        self.inner.compact_f32_available(n)
    }
    fn compact_f32(&self, v: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        self.inner.compact_f32(v)
    }
    fn store_compact_f32(&mut self, b: &[u8], n: u32) -> Result<ValueRef, StorageError> {
        let v = self.inner.store_compact_f32(b, n)?;
        self.compact += 1;
        self.physical += b.len() as u64;
        Ok(v)
    }
}
