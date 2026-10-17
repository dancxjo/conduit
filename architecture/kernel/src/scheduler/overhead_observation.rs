//! Test-only observations around the real fixed kernel, never a product allocator.
extern crate std;
use crate::{FixedValueStore, StorageError, ValueRef, ValueStorage};
use core::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Allocations {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    pub requested_bytes: u64,
}
std::thread_local! {
    static SAMPLE: Cell<Option<Allocations>> = const { Cell::new(None) };
}
struct ObservedAllocator;
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;
fn observe(update: impl FnOnce(&mut Allocations)) {
    let _ = SAMPLE.try_with(|sample| {
        if let Some(mut counts) = sample.get() {
            update(&mut counts);
            sample.set(Some(counts));
        }
    });
}
// SAFETY: every allocation operation delegates unchanged to System. The const
// thread-local Cell retains only counters, allocates no storage and holds no
// borrowed allocation. Other test threads cannot contaminate a sample.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            observe(|c| {
                c.allocations += 1;
                c.requested_bytes += layout.size() as u64;
            });
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            observe(|c| {
                c.allocations += 1;
                c.requested_bytes += layout.size() as u64;
            });
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        observe(|c| c.deallocations += 1);
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let output = unsafe { System.realloc(pointer, layout, size) };
        if !output.is_null() {
            observe(|c| {
                c.reallocations += 1;
                c.requested_bytes += size as u64;
            });
        }
        output
    }
}
pub(super) struct AllocationSample;
impl AllocationSample {
    pub fn begin() -> Self {
        SAMPLE.with(|s| {
            assert!(
                s.replace(Some(Allocations::default())).is_none(),
                "nested allocation sample"
            )
        });
        Self
    }
    pub fn finish(self) -> Allocations {
        SAMPLE.with(|s| s.replace(None).expect("active allocation sample"))
    }
}
impl Drop for AllocationSample {
    fn drop(&mut self) {
        let _ = SAMPLE.try_with(|s| s.set(None));
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ValueAccess {
    pub successful_stores: u64,
    /// Exact bytes passed to successful FixedValueStore::store calls. The
    /// fixed store performs one copy_from_slice per successful admitted write.
    /// This does not count compiler moves, Sign storage or Host transport copies.
    pub stored_bytes: u64,
    pub borrowed_reads: u64,
}
pub(super) struct ObservedValues<const N: usize> {
    inner: FixedValueStore<N, 1>,
    access: Cell<ValueAccess>,
}
impl<const N: usize> ObservedValues<N> {
    pub fn new() -> Self {
        Self {
            inner: FixedValueStore::new(N as u32).unwrap(),
            access: Cell::new(ValueAccess::default()),
        }
    }
    pub fn access(&self) -> ValueAccess {
        self.access.get()
    }
}
impl<const N: usize> ValueStorage for ObservedValues<N> {
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
        let mut counts = self.access.get();
        counts.successful_stores += 1;
        counts.stored_bytes += bytes.len() as u64;
        self.access.set(counts);
        Ok(value)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        let bytes = self.inner.get(value)?;
        let mut counts = self.access.get();
        counts.borrowed_reads += 1;
        self.access.set(counts);
        Ok(bytes)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.release(value)
    }
    fn clear(&mut self) {
        self.inner.clear();
    }
}
#[test]
fn allocator_probe_counts_real_storage_without_other_thread_noise() {
    let sample = AllocationSample::begin();
    let mut bytes = std::vec::Vec::<u8>::with_capacity(31);
    std::hint::black_box(&mut bytes).push(1);
    drop(bytes);
    let counts = sample.finish();
    assert_eq!(counts.allocations, 1);
    assert_eq!(counts.deallocations, 1);
    assert_eq!(counts.reallocations, 0);
    assert_eq!(counts.requested_bytes, 31);
}
#[test]
fn value_observation_delegates_custody_and_counts_only_successful_copies() {
    let mut values = ObservedValues::<1>::new();
    let value = values.store(&[7]).unwrap();
    assert_eq!(values.store(&[8]), Err(StorageError::ByteCapacityExceeded));
    assert_eq!(values.get(value).unwrap(), &[7]);
    let counts = values.access();
    assert_eq!(counts.successful_stores, 1);
    assert_eq!(counts.stored_bytes, 1);
    assert_eq!(counts.borrowed_reads, 1);
    values.release(value).unwrap();
    assert_eq!(values.get(value), Err(StorageError::StaleReference));
}
