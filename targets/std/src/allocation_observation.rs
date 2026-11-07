//! Optional process-wide System allocator diagnostics. These counters observe
//! requested Rust backing only; they neither grant memory nor contain effects.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REQUESTED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LIVE_BLOCKS: AtomicU64 = AtomicU64::new(0);
static PEAK_BLOCKS: AtomicU64 = AtomicU64::new(0);

/// Select explicitly as the executable's global allocator to observe from its
/// first Rust allocation. Instantiating this type does not activate any grant.
pub struct SystemAllocationObserver;

fn allocated(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(live, Relaxed);
    ALLOCATIONS.fetch_add(1, Relaxed);
    PEAK_BLOCKS.fetch_max(LIVE_BLOCKS.fetch_add(1, Relaxed) + 1, Relaxed);
    REQUESTED.fetch_add(bytes as u64, Relaxed);
}

// SAFETY: pointer ownership, allocation layouts and refusal are delegated
// unchanged to System. Diagnostic bookkeeping uses fixed atomics, no allocation.
unsafe impl GlobalAlloc for SystemAllocationObserver {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        DEALLOCATIONS.fetch_add(1, Relaxed);
        LIVE_BLOCKS.fetch_sub(1, Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, bytes) };
        if !next.is_null() {
            // Conservatively include both old and new backing during realloc,
            // even if System happened to resize this particular block in place.
            let prior = LIVE.fetch_add(bytes, Relaxed);
            PEAK.fetch_max(prior.saturating_add(bytes), Relaxed);
            LIVE.fetch_sub(layout.size(), Relaxed);
            REALLOCATIONS.fetch_add(1, Relaxed);
            REQUESTED.fetch_add(bytes as u64, Relaxed);
        }
        next
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct ProcessAllocationObservation {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    pub requested_bytes: u64,
    pub live_requested_bytes: usize,
    pub peak_requested_bytes: usize,
    pub live_allocation_blocks: u64,
    pub peak_allocation_blocks: u64,
}

/// Observational snapshot, not an atomic census or admission witness. Counts
/// cover successful Rust allocator calls across all threads since process start.
/// They exclude allocator metadata, C allocations, mappings, stacks and children.
pub fn snapshot() -> ProcessAllocationObservation {
    ProcessAllocationObservation {
        allocations: ALLOCATIONS.load(Relaxed),
        reallocations: REALLOCATIONS.load(Relaxed),
        deallocations: DEALLOCATIONS.load(Relaxed),
        requested_bytes: REQUESTED.load(Relaxed),
        live_requested_bytes: LIVE.load(Relaxed),
        peak_requested_bytes: PEAK.load(Relaxed),
        live_allocation_blocks: LIVE_BLOCKS.load(Relaxed),
        peak_allocation_blocks: PEAK_BLOCKS.load(Relaxed),
    }
}
