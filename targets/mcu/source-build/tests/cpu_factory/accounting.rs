//! CPU allocation accounting. Simulated external NOR bytes are not RAM custody.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static BASELINE: AtomicUsize = AtomicUsize::new(0);
static EXTERNAL_MEDIA: AtomicUsize = AtomicUsize::new(0);
pub struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            CURRENT.fetch_add(layout.size(), SeqCst);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, new) };
        if !result.is_null() {
            CURRENT.fetch_add(new, SeqCst);
            CURRENT.fetch_sub(layout.size(), SeqCst);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
pub fn begin() {
    BASELINE.store(CURRENT.load(SeqCst), SeqCst);
}
pub fn exclude_media(bytes: usize) {
    EXTERNAL_MEDIA.fetch_add(bytes, SeqCst);
}
pub fn used() -> usize {
    CURRENT
        .load(SeqCst)
        .saturating_sub(BASELINE.load(SeqCst))
        .saturating_sub(EXTERNAL_MEDIA.load(SeqCst))
}
