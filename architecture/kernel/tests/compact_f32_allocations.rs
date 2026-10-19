#![cfg(feature = "alloc")]
use conduit_kernel::{HostedValueStore, ValueStorage};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
}
struct Allocator;
fn record(index: usize) {
    let _ = ACTIVE.try_with(|active| {
        if active.get() {
            REQUESTS.with(|requests| {
                let mut counts = requests.get();
                counts[index] += 1;
                requests.set(counts);
            });
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            record(0);
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            record(0);
        }
        result
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            record(1);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(2);
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn prepared_compact_publication_fanout_and_reuse_make_no_allocator_requests() {
    let mut values = HostedValueStore::new(2, 1372, 2744).unwrap();
    values.prepare_compact_f32();
    let capacity = values.allocation_capacities();
    let backing = values.reserved_storage_bytes();
    let lanes = [0u8; 512];
    REQUESTS.with(|counts| counts.set([0; 3]));
    ACTIVE.with(|active| active.set(true));
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(false));
        }
    }
    let guard = Guard;
    for _ in 0..128 {
        let value = values.store_compact_f32(&lanes, 1372).unwrap();
        assert_eq!(values.used_bytes(), 1372);
        values.retain(value).unwrap();
        values.release(value).unwrap();
        assert_eq!(values.compact_f32(value), Ok(Some(lanes.as_slice())));
        values.release(value).unwrap();
        let boundary = values.store(b"canonical boundary").unwrap();
        assert_eq!(values.compact_f32(boundary), Ok(None));
        values.clear();
    }
    drop(guard);
    assert_eq!(REQUESTS.with(Cell::get), [0; 3]);
    assert_eq!(values.allocation_capacities(), capacity);
    assert_eq!(values.reserved_storage_bytes(), backing);
}
