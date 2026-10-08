//! Actual complete bank preparation allocation probe. No production allocator.
use conduit_language::parser_window8_program_bank::*;
use conduit_plot::rust_binding::PreparedNativeFamilyLimits;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::Ordering;
use std::{cell::Cell, thread::LocalKey};
// Tests can run concurrently. Counters observe only the allocating test thread;
// all values measured by these probes are created and dropped on that thread.
// No production allocator or runtime behavior is changed.
std::thread_local! {
 static LIVE_COUNT:Cell<isize>=const{Cell::new(0)};
 static PEAK_COUNT:Cell<isize>=const{Cell::new(0)};
 static TRACK_FLAG:Cell<bool>=const{Cell::new(false)};
}
struct LocalCounter(&'static LocalKey<Cell<isize>>);
impl LocalCounter {
    fn load(&self, _: Ordering) -> isize {
        self.0.try_with(Cell::get).unwrap_or(0)
    }
    fn store(&self, value: isize, _: Ordering) {
        let _ = self.0.try_with(|cell| cell.set(value));
    }
    fn fetch_add(&self, value: isize, _: Ordering) -> isize {
        self.0
            .try_with(|cell| {
                let old = cell.get();
                cell.set(old + value);
                old
            })
            .unwrap_or(0)
    }
    fn fetch_sub(&self, value: isize, order: Ordering) -> isize {
        self.fetch_add(-value, order)
    }
    fn fetch_max(&self, value: isize, _: Ordering) -> isize {
        self.0
            .try_with(|cell| {
                let old = cell.get();
                cell.set(old.max(value));
                old
            })
            .unwrap_or(0)
    }
}
struct LocalFlag;
impl LocalFlag {
    fn load(&self, _: Ordering) -> bool {
        TRACK_FLAG.try_with(Cell::get).unwrap_or(false)
    }
    fn store(&self, value: bool, _: Ordering) {
        let _ = TRACK_FLAG.try_with(|cell| cell.set(value));
    }
}
static LIVE: LocalCounter = LocalCounter(&LIVE_COUNT);
static PEAK: LocalCounter = LocalCounter(&PEAK_COUNT);
static TRACK: LocalFlag = LocalFlag;
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            let live =
                LIVE.fetch_add(layout.size() as isize, Ordering::SeqCst) + layout.size() as isize;
            if TRACK.load(Ordering::SeqCst) {
                PEAK.fetch_max(live, Ordering::SeqCst);
            }
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, size);
        if !result.is_null() {
            let live = LIVE.fetch_add(size as isize - layout.size() as isize, Ordering::SeqCst)
                + size as isize
                - layout.size() as isize;
            if TRACK.load(Ordering::SeqCst) {
                PEAK.fetch_max(live, Ordering::SeqCst);
            }
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;

#[test]
fn prepared_source_complete_bank_bound_covers_measured_requests() {
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let bank = Window8ProgramBank::prepare_native_evaluator(
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 64,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 256 * 1024 * 1024,
            maximum_preparation_peak_bytes: 512 * 1024 * 1024,
            maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
        },
        Window8SourcePreparationLimits {
            maximum_retained_bytes: 1024 * 1024 * 1024,
            maximum_preparation_peak_bytes: 2 * 1024 * 1024 * 1024,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        },
    )
    .unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let observed_peak = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let observed_retained = (LIVE.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let receipt = bank.prepared_source_receipt().unwrap();
    assert!(observed_peak <= receipt.preparation_peak_heap_bytes_bound);
    assert!(observed_retained <= receipt.retained_heap_bytes_bound);
    eprintln!("complete preparedSource bank measured_peak={observed_peak} measured_retained={observed_retained} receipt={receipt:?}; requested allocations, no allocator bookkeeping");
}
