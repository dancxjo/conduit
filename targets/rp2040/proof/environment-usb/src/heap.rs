//! Fixed diagnostic Face-construction heap; no authority or storage journal.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
};
use critical_section::Mutex;
use linked_list_allocator::Heap;
const CAPACITY: usize = crate::config::HEAP_BYTES;
#[cfg(feature = "zenoh-evaluation")]
const FOREIGN_BYTES: usize = 16 * 1024;
#[cfg(not(feature = "zenoh-evaluation"))]
const FOREIGN_BYTES: usize = 0;
const RUST_BYTES: usize = CAPACITY - FOREIGN_BYTES;
#[cfg(feature = "zenoh-evaluation")]
const _: () = assert!(RUST_BYTES >= 16 * 1024);
#[cfg(feature = "zenoh-evaluation")]
pub type ForeignBacking = &'static mut [core::mem::MaybeUninit<u8>];
#[cfg(not(feature = "zenoh-evaluation"))]
pub type ForeignBacking = ();
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
pub unsafe fn initialize() -> ForeignBacking {
    critical_section::with(|cs| unsafe {
        ALLOCATOR
            .0
            .borrow(cs)
            .borrow_mut()
            .heap
            .init(core::ptr::addr_of_mut!(BYTES).cast(), RUST_BYTES);
    });
    #[cfg(feature = "zenoh-evaluation")]
    unsafe {
        // This disjoint suffix never enters the Rust global allocator.
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(BYTES)
                .cast::<u8>()
                .add(RUST_BYTES)
                .cast(),
            FOREIGN_BYTES,
        )
    }
}
#[cfg(feature = "zenoh-evaluation")]
pub fn capacity() -> usize {
    RUST_BYTES
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
