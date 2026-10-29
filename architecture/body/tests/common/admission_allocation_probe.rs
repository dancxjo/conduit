//! Actual System allocator accounting and one thread-local null refusal for admission storage tests.
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
    pub fn retained_bytes(&self) -> usize {
        self.live_bytes
    }
}
thread_local! {
    static REFUSE_BYTES: Cell<Option<usize>> = const { Cell::new(None) };
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static OBSERVATION: Cell<Observation> = const { Cell::new(Observation { allocations: 0, reallocations: 0, deallocations: 0, deallocated_bytes: 0, peak_bytes: 0, requested_bytes: 0, live_bytes: 0 }) };
}
pub struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0, false);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            return core::ptr::null_mut();
        }
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
        if refuse(size) {
            return core::ptr::null_mut();
        }
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

fn refuse(bytes: usize) -> bool {
    REFUSE_BYTES
        .try_with(|expected| {
            if expected.get() == Some(bytes) {
                expected.set(None);
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}
pub struct RefusalGuard;
impl Drop for RefusalGuard {
    fn drop(&mut self) {
        REFUSE_BYTES.with(|expected| expected.set(None));
    }
}
pub fn refuse_next_allocation_size(bytes: usize) -> RefusalGuard {
    assert!(bytes > 0);
    REFUSE_BYTES.with(|expected| assert!(expected.replace(Some(bytes)).is_none()));
    RefusalGuard
}
