//! Fixed diagnostic Face-construction heap; no authority or storage journal.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
};
use critical_section::Mutex;
use linked_list_allocator::Heap;
const CAPACITY: usize = 48 * 1024;
static mut BYTES: [u8; CAPACITY] = [0; CAPACITY];
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
            .init(core::ptr::addr_of_mut!(BYTES).cast(), CAPACITY);
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
        });
    }
}
