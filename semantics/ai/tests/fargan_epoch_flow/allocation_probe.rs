//! Thread-local development measurement; never part of a product owner.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
struct Probe;
#[global_allocator]
static ALLOCATOR: Probe = Probe;
fn observed() {
    if ACTIVE.try_with(Cell::get).unwrap_or(false) {
        COUNT.with(|count| count.set(count.get() + 1));
    }
}
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        observed();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        observed();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        observed();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
pub(super) fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    COUNT.with(|count| count.set(0));
    ACTIVE.with(|active| assert!(!active.replace(true)));
    let result = operation();
    ACTIVE.with(|active| active.set(false));
    (result, COUNT.with(Cell::get))
}
