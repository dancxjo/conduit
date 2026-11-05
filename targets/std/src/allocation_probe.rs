//! Thread-local test allocator; probes current-thread allocation and reallocation.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record() {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            let _ = ALLOCATIONS.try_with(|allocations| {
                allocations.set(allocations.get().saturating_add(1));
            });
        }
    });
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record();
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record();
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !pointer.is_null() {
            record();
        }
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

pub struct Guard {
    finished: bool,
}

impl Guard {
    pub fn finish(mut self) -> usize {
        self.finished = true;
        TRACKING.with(|tracking| tracking.set(false));
        ALLOCATIONS.with(Cell::get)
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
    ALLOCATIONS.with(|allocations| allocations.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    Guard { finished: false }
}
