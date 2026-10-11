//! Counts allocations on the calling test thread only.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static LIVE: Cell<Option<i64>> = const { Cell::new(None) };
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

fn rounded_live(bytes: usize, adding: bool) {
    let rounded = bytes.saturating_add(31) / 32 * 32;
    let _ = LIVE.try_with(|live| {
        if let Some(value) = live.get() {
            live.set(Some(if adding {
                value + rounded as i64
            } else {
                value - rounded as i64
            }));
        }
    });
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
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            rounded_live(layout.size(), true);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: The caller supplies the allocator's valid Layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            rounded_live(layout.size(), true);
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: The caller owns the System allocation and valid new size.
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            rounded_live(layout.size(), false);
            rounded_live(size, true);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller owns the System allocation with this Layout.
        rounded_live(layout.size(), false);
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

/// Net live requested storage rounded to BootArena's 32-byte allocation units.
/// The returned object remains alive until after measurement; transient storage
/// released within the operation is excluded. This models allocation-unit charge,
/// not host System allocator bookkeeping or an actual initialized boot arena.
#[allow(dead_code)] // Optional measurement for tests sharing this allocator.
pub fn rounded_retained_bytes<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            LIVE.with(|live| live.set(None));
        }
    }
    LIVE.with(|live| assert!(live.replace(Some(0)).is_none()));
    let reset = Reset;
    let result = operation();
    let bytes = LIVE.with(|live| live.get().unwrap());
    assert!(
        bytes >= 0,
        "measurement released more preexisting storage than it retained"
    );
    drop(reset);
    (result, bytes as usize)
}
