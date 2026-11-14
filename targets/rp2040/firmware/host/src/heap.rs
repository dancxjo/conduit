//! Fixed checked Host preparation arena; no resource grant or storage journal.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};
use critical_section::Mutex;
use linked_list_allocator::Heap;
const CAPACITY: usize = crate::config::HEAP_BYTES;
const RUST_BYTES: usize = CAPACITY;
static mut BYTES: [u8; CAPACITY] = [0; CAPACITY];
static LAST_USED: AtomicUsize = AtomicUsize::new(0);
static LAST_PEAK: AtomicUsize = AtomicUsize::new(0);
/// Recovery observation only: never borrow allocator state from a panic path.
pub fn observed_usage() -> (usize, usize) {
    (
        LAST_USED.load(Ordering::Relaxed),
        LAST_PEAK.load(Ordering::Relaxed),
    )
}
struct State {
    heap: Heap,
    peak: usize,
}
struct Allocator(Mutex<RefCell<State>>);
#[global_allocator]
static ALLOCATOR: Allocator = Allocator(Mutex::new(RefCell::new(State {
    heap: Heap::empty(),
    peak: 0,
})));
pub unsafe fn initialize() {
    critical_section::with(|cs| unsafe {
        ALLOCATOR
            .0
            .borrow(cs)
            .borrow_mut()
            .heap
            .init(core::ptr::addr_of_mut!(BYTES).cast(), RUST_BYTES);
    });
}
pub fn usage() -> (usize, usize) {
    critical_section::with(|cs| {
        let s = ALLOCATOR.0.borrow(cs).borrow();
        (s.heap.used(), s.peak)
    })
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        critical_section::with(|cs| {
            let mut s = ALLOCATOR.0.borrow(cs).borrow_mut();
            let pointer = s
                .heap
                .allocate_first_fit(layout)
                .map_or(core::ptr::null_mut(), |p| p.as_ptr());
            s.peak = s.peak.max(s.heap.used());
            LAST_USED.store(s.heap.used(), Ordering::Relaxed);
            LAST_PEAK.store(s.peak, Ordering::Relaxed);
            pointer
        })
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        critical_section::with(|cs| unsafe {
            ALLOCATOR
                .0
                .borrow(cs)
                .borrow_mut()
                .heap
                .deallocate(core::ptr::NonNull::new_unchecked(pointer), layout);
            LAST_USED.store(
                ALLOCATOR.0.borrow(cs).borrow().heap.used(),
                Ordering::Relaxed,
            );
        });
    }
}
