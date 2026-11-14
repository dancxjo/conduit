//! Thread-local test allocator; probes current-thread allocation and reallocation.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

pub struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static BYTES: RefCell<ByteProbe> = RefCell::new(ByteProbe::default());
}

/// Requested Rust allocation bytes on this thread only. Excludes allocator
/// metadata, probe bookkeeping, preexisting allocations and other threads or
/// processes. Reallocation peak conservatively includes both old and new blocks.
/// These observations are diagnostic, never a complete admission qualification.
#[derive(Clone, Copy, Debug, Default)]
pub struct Statistics {
    pub allocations: usize,
    pub reallocations: usize,
    pub requested_bytes: usize,
    pub live_bytes: usize,
    pub peak_live_bytes: usize,
}
#[derive(Default)]
struct ByteProbe {
    pointers: HashMap<usize, usize>,
    statistics: Statistics,
}
fn record(old: Option<usize>, new: Option<(usize, usize)>) {
    let _ = TRACKING.try_with(|tracking| {
        if !tracking.replace(false) {
            return;
        }
        // Bookkeeping can allocate; suppress reentrant profiling of itself.
        let _ = BYTES.try_with(|bytes| {
            let mut probe = bytes.borrow_mut();
            let before = probe.statistics.live_bytes;
            if let Some(old) = old {
                if let Some(size) = probe.pointers.remove(&old) {
                    probe.statistics.live_bytes -= size;
                }
            }
            if let Some((pointer, size)) = new {
                probe.statistics.reallocations += usize::from(old.is_some());
                if let Some(previous) = probe.pointers.insert(pointer, size) {
                    probe.statistics.live_bytes -= previous;
                }
                probe.statistics.live_bytes = probe.statistics.live_bytes.saturating_add(size);
                probe.statistics.requested_bytes =
                    probe.statistics.requested_bytes.saturating_add(size);
                probe.statistics.peak_live_bytes = probe
                    .statistics
                    .peak_live_bytes
                    .max(before.saturating_add(size))
                    .max(probe.statistics.live_bytes);
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get().saturating_add(1)));
            }
        });
        tracking.set(true);
    });
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(None, Some((pointer as usize, layout.size())));
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record(None, Some((pointer as usize, layout.size())));
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(Some(pointer as usize), None);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = pointer as usize;
        let pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !pointer.is_null() {
            record(Some(old), Some((pointer as usize, new_size)));
        }
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

pub struct Guard {
    finished: bool,
    // A thread-local measurement must finish on the thread that began it.
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Guard {
    pub fn finish(self) -> usize {
        self.finish_statistics().allocations
    }

    pub fn finish_statistics(mut self) -> Statistics {
        self.finished = true;
        TRACKING.with(|tracking| tracking.set(false));
        let mut statistics = BYTES.with(|bytes| bytes.borrow().statistics);
        statistics.allocations = ALLOCATIONS.with(Cell::get);
        statistics
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
}

pub fn current() -> usize {
    ALLOCATIONS.with(Cell::get)
}

pub fn begin() -> Guard {
    TRACKING.with(|tracking| assert!(!tracking.get(), "allocation probes cannot nest"));
    BYTES.with(|bytes| {
        let mut probe = bytes.borrow_mut();
        probe.pointers.clear();
        probe.statistics = Statistics::default();
    });
    ALLOCATIONS.with(|allocations| allocations.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    Guard {
        finished: false,
        _thread: std::marker::PhantomData,
    }
}

/// Shared by test binaries that prefer a callback-delimited measurement.
#[allow(dead_code)] // Other binaries use the explicit interval/count interface.
pub fn observe<T>(work: impl FnOnce() -> T) -> (T, Statistics) {
    let probe = begin();
    let value = work();
    (value, probe.finish_statistics())
}

#[cfg(test)]
mod byte_tests {
    #[test]
    fn count_only_callers_keep_the_existing_interface() {
        let probe = super::begin();
        let bytes = std::hint::black_box(vec![0u8; 128]);
        assert_eq!(super::current(), 1);
        assert_eq!(probe.finish(), 1);
        std::hint::black_box(bytes);
    }

    #[test]
    fn preexisting_backing_is_not_charged_and_reallocation_is_conservative() {
        let old = std::hint::black_box(vec![0u8; 4096]);
        let probe = super::begin();
        drop(old);
        let mut bytes = std::hint::black_box(vec![0u8; 32]);
        bytes.reserve_exact(64);
        std::hint::black_box(&bytes);
        let statistics = probe.finish_statistics();
        assert_eq!(statistics.allocations, 2);
        assert_eq!(statistics.reallocations, 1);
        assert_eq!(statistics.live_bytes, bytes.capacity());
        assert_eq!(statistics.requested_bytes, 32 + bytes.capacity());
        assert_eq!(statistics.peak_live_bytes, 32 + bytes.capacity());
    }

    #[test]
    fn live_requested_bytes_track_release_without_counting_probe_storage() {
        let probe = super::begin();
        let first = std::hint::black_box(vec![0u8; 1024]);
        let second = std::hint::black_box(vec![0u8; 2048]);
        drop(first);
        let statistics = probe.finish_statistics();
        assert_eq!(statistics.allocations, 2);
        assert_eq!(statistics.requested_bytes, 3072);
        assert_eq!(statistics.live_bytes, 2048);
        assert_eq!(statistics.peak_live_bytes, 3072);
        drop(second);
    }
}
