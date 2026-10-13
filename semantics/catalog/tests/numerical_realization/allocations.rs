//! Count allocations only on the thread executing the admitted Play.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
struct CountedSystem;
fn record() {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        COUNT.with(|count| count.set(count.get() + 1));
    }
}
unsafe impl GlobalAlloc for CountedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        System.realloc(ptr, layout, size)
    }
}
#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;
pub fn start() {
    COUNT.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
}
pub fn finish() -> usize {
    TRACK.with(|c| c.set(false));
    COUNT.with(Cell::get)
}
