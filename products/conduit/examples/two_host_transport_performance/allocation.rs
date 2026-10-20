//! Process-wide Rust allocation requests, including the joined sink Host thread.
use serde::Serialize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static REALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCS: AtomicU64 = AtomicU64::new(0);
static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
pub struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() && ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() && ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() && ACTIVE.load(Ordering::Relaxed) {
            REALLOCS.fetch_add(1, Ordering::Relaxed);
            REALLOC_BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ACTIVE.load(Ordering::Relaxed) {
            DEALLOCS.fetch_add(1, Ordering::Relaxed);
            DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[derive(Serialize)]
pub struct Stats {
    pub elapsed_ns: u64,
    pub allocations: u64,
    pub allocated_request_bytes: u64,
    pub reallocations: u64,
    pub reallocated_request_bytes: u64,
    pub deallocations: u64,
    pub deallocated_request_bytes: u64,
}
pub fn observe<T>(work: impl FnOnce() -> T) -> (T, Stats) {
    assert!(!ACTIVE.load(Ordering::SeqCst), "nested allocation scope");
    for counter in [
        &ALLOCS,
        &ALLOC_BYTES,
        &REALLOCS,
        &REALLOC_BYTES,
        &DEALLOCS,
        &DEALLOC_BYTES,
    ] {
        counter.store(0, Ordering::SeqCst);
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ACTIVE.store(false, Ordering::SeqCst);
        }
    }
    let guard = Guard;
    let started = Instant::now();
    ACTIVE.store(true, Ordering::SeqCst);
    let result = work();
    ACTIVE.store(false, Ordering::SeqCst);
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos()).expect("bounded capture clock");
    drop(guard);
    (
        result,
        Stats {
            elapsed_ns,
            allocations: ALLOCS.load(Ordering::SeqCst),
            allocated_request_bytes: ALLOC_BYTES.load(Ordering::SeqCst),
            reallocations: REALLOCS.load(Ordering::SeqCst),
            reallocated_request_bytes: REALLOC_BYTES.load(Ordering::SeqCst),
            deallocations: DEALLOCS.load(Ordering::SeqCst),
            deallocated_request_bytes: DEALLOC_BYTES.load(Ordering::SeqCst),
        },
    )
}
