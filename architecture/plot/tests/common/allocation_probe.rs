use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct Observation {
    pub allocations: usize,
    pub reallocations: usize,
    pub deallocations: usize,
    pub deallocated_bytes: usize,
    pub peak_bytes: usize,
    pub requested_bytes: usize,
    live_bytes: usize,
}
impl Observation {
    #[allow(
        dead_code,
        reason = "shared probe; only compaction tests inspect retained payload"
    )]
    pub fn retained_bytes(&self) -> usize {
        self.live_bytes
    }
}
thread_local! {
    static REFUSE_ALLOCATION_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static OBSERVATION: Cell<Observation> = const { Cell::new(Observation { allocations: 0, reallocations: 0, deallocations: 0, deallocated_bytes: 0, peak_bytes: 0, requested_bytes: 0, live_bytes: 0 }) };
}
pub struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if REFUSE_ALLOCATION_SIZE
            .try_with(|size| {
                if size.get() == Some(layout.size()) {
                    size.set(None);
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false)
        {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0, false);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0, false);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        update(0, layout.size(), false);
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            update(size, layout.size(), true);
        }
        next
    }
}
fn update(added: usize, removed: usize, reallocation: bool) {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            OBSERVATION.with(|observation| {
                let mut state = observation.get();
                if reallocation {
                    state.reallocations += 1;
                } else if added != 0 {
                    state.allocations += 1;
                } else if removed != 0 {
                    state.deallocations += 1;
                    state.deallocated_bytes += removed;
                }
                state.live_bytes = state
                    .live_bytes
                    .saturating_sub(removed)
                    .saturating_add(added);
                state.requested_bytes = state.requested_bytes.saturating_add(added);
                state.peak_bytes = state.peak_bytes.max(state.live_bytes);
                observation.set(state);
            });
        }
    });
}
pub fn observe<T>(work: impl FnOnce() -> T) -> (T, Observation) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
    TRACKING.with(|tracking| assert!(!tracking.replace(true)));
    OBSERVATION.with(|observation| observation.set(Observation::default()));
    let guard = Guard;
    let result = work();
    drop(guard);
    let observation = OBSERVATION.with(Cell::get);
    (result, observation)
}

/// Thread-scoped allocator refusal for the actual fallible allocation path.
/// Drop clears an unconsumed request so it cannot affect another phase/test.
#[allow(dead_code)]
pub struct AllocationRefusal;
impl Drop for AllocationRefusal {
    fn drop(&mut self) {
        REFUSE_ALLOCATION_SIZE.with(|size| size.set(None));
    }
}
#[allow(dead_code)]
pub fn refuse_next_allocation_size(bytes: usize) -> AllocationRefusal {
    REFUSE_ALLOCATION_SIZE.with(|size| assert!(size.replace(Some(bytes)).is_none()));
    AllocationRefusal
}
