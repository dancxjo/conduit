use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

std::thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ARMED.with(|armed| {
            if armed.get() {
                COUNT.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ARMED.with(|armed| {
            if armed.get() {
                COUNT.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static TEST_ALLOCATOR: CountingAllocator = CountingAllocator;

pub fn allocations_during(run: impl FnOnce()) -> usize {
    struct Disarm;
    impl Drop for Disarm {
        fn drop(&mut self) {
            ARMED.with(|armed| armed.set(false));
        }
    }

    COUNT.with(|count| count.set(0));
    ARMED.with(|armed| armed.set(true));
    let disarm = Disarm;
    run();
    drop(disarm);
    COUNT.with(Cell::get)
}

#[allow(dead_code)]
pub fn assert_no_allocations(label: &str, run: impl FnOnce()) {
    let allocations = allocations_during(run);
    assert_eq!(allocations, 0, "{label} allocated {allocations} times");
}
