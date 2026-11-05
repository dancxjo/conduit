//! Thread-scoped allocation observation and deterministic next-allocation refusal.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub requests: usize,
    pub allocations: usize,
    pub deallocations: usize,
}
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static FAIL: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<Counts> = const { Cell::new(Counts {requests:0, allocations:0, deallocations:0}) };
}
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let fail = ACTIVE
            .try_with(|active| {
                if !active.get() {
                    return false;
                }
                COUNTS.with(|counts| {
                    let mut c = counts.get();
                    c.requests += 1;
                    counts.set(c);
                });
                FAIL.with(|fail| fail.replace(false))
            })
            .unwrap_or(false);
        if fail {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let _ = ACTIVE.try_with(|a| {
                if a.get() {
                    COUNTS.with(|counts| {
                        let mut c = counts.get();
                        c.allocations += 1;
                        counts.set(c);
                    });
                }
            });
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let _ = ACTIVE.try_with(|a| {
            if a.get() {
                COUNTS.with(|counts| {
                    let mut c = counts.get();
                    c.deallocations += 1;
                    counts.set(c);
                });
            }
        });
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
}
pub fn observe<T>(fail: bool, work: impl FnOnce() -> T) -> (T, Counts) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ACTIVE.with(|a| a.set(false));
            FAIL.with(|a| a.set(false));
        }
    }
    ACTIVE.with(|a| assert!(!a.replace(true)));
    FAIL.with(|a| a.set(fail));
    COUNTS.with(|a| a.set(Counts::default()));
    let guard = Guard;
    let result = work();
    drop(guard);
    (result, COUNTS.with(Cell::get))
}
