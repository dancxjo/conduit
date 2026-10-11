//! Actual process allocator counters. Only allocations are instrumented; warm
//! arithmetic and selection do not gain a counter operation when they allocate none.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static DEALLOCS: AtomicU64 = AtomicU64::new(0);
static REQUESTED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
pub struct Allocator;
fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(live, Relaxed);
}
fn allocation(bytes: usize) {
    grow(bytes);
    if ACTIVE.load(Relaxed) {
        ALLOCS.fetch_add(1, Relaxed);
        REQUESTED.fetch_add(bytes as u64, Relaxed);
    }
}
// Delegate ownership and layouts exactly to System. Counts describe successful
// allocations; failure is returned unchanged to the caller.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocation(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocation(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ACTIVE.load(Relaxed) {
            DEALLOCS.fetch_add(1, Relaxed);
        }
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let output = unsafe { System.realloc(pointer, layout, size) };
        if !output.is_null() {
            if size >= layout.size() {
                grow(size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - size, Relaxed);
            }
            if ACTIVE.load(Relaxed) {
                REALLOCS.fetch_add(1, Relaxed);
                REQUESTED.fetch_add(size as u64, Relaxed);
            }
        }
        output
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Counts {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    pub requested_bytes: u64,
    pub live_heap_bytes_at_end: usize,
    pub phase_peak_heap_bytes: usize,
}
pub fn start() {
    assert!(!ACTIVE.load(Relaxed), "nested allocation measurement");
    for counter in [&ALLOCS, &REALLOCS, &DEALLOCS, &REQUESTED] {
        counter.store(0, Relaxed);
    }
    PEAK.store(LIVE.load(Relaxed), Relaxed);
    ACTIVE.store(true, Relaxed);
}
pub fn finish() -> Counts {
    ACTIVE.store(false, Relaxed);
    Counts {
        allocations: ALLOCS.load(Relaxed),
        reallocations: REALLOCS.load(Relaxed),
        deallocations: DEALLOCS.load(Relaxed),
        requested_bytes: REQUESTED.load(Relaxed),
        live_heap_bytes_at_end: LIVE.load(Relaxed),
        phase_peak_heap_bytes: PEAK.load(Relaxed),
    }
}
