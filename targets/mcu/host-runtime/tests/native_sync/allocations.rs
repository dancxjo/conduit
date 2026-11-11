//! Per-thread allocation attempts during already-prepared synchronous dispatch.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
std::thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
}
struct Tracked;
fn note() {
    let _ = TRACK.try_with(|track| {
        if track.get() {
            let _ = ATTEMPTS.try_with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: all allocations/deallocations delegate unchanged to the System allocator.
unsafe impl GlobalAlloc for Tracked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, n: usize) -> *mut u8 {
        note();
        unsafe { System.realloc(ptr, layout, n) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Tracked = Tracked;
pub fn without_allocations<T>(work: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACK.with(|track| track.set(false));
        }
    }
    ATTEMPTS.with(|count| count.set(0));
    TRACK.with(|track| assert!(!track.replace(true)));
    let guard = Guard;
    let result = work();
    drop(guard);
    assert_eq!(ATTEMPTS.with(Cell::get), 0, "prepared dispatch allocated");
    result
}
