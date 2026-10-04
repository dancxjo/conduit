//! Counts allocations on the calling test thread only.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

struct Counting;
#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn count() {
    let _ = COUNT.try_with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}

// SAFETY: Every allocation operation delegates its unchanged arguments and
// result to System. The thread-local counter owns no allocated storage.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: The caller supplies the allocator's valid Layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: The caller supplies the allocator's valid Layout.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: The caller owns the System allocation and valid new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller owns the System allocation with this Layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}

pub fn allocations(operation: impl FnOnce()) -> usize {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNT.with(|count| count.set(None));
        }
    }
    COUNT.with(|count| assert!(count.replace(Some(0)).is_none()));
    let reset = Reset;
    operation();
    let result = COUNT.with(|count| count.get().unwrap());
    drop(reset);
    result
}
