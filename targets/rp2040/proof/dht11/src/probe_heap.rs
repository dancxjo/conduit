//! Measured, finite preparation heap for this diagnostic only.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    ptr::NonNull,
};
use critical_section::Mutex;
use linked_list_allocator::Heap;

#[cfg(not(feature = "kernel-evaluation"))]
pub const CAPACITY: usize = 96 * 1024;
#[cfg(all(feature = "kernel-evaluation", not(feature = "http-ap")))]
pub const CAPACITY: usize = 196 * 1024;
#[cfg(feature = "http-ap")]
pub const CAPACITY: usize = 196 * 1024;
static mut BYTES: [u8; CAPACITY] = [0; CAPACITY];
struct State {
    heap: Heap,
    peak: usize,
    allocations: usize,
    sealed: bool,
}
pub struct ProbeHeap(Mutex<RefCell<State>>);
#[global_allocator]
pub static ALLOCATOR: ProbeHeap = ProbeHeap(Mutex::new(RefCell::new(State {
    heap: Heap::empty(),
    peak: 0,
    allocations: 0,
    sealed: false,
})));

impl ProbeHeap {
    /// Called once, before any allocation or executor tasks start.
    pub unsafe fn initialize(&self) {
        critical_section::with(|cs| unsafe {
            self.0
                .borrow(cs)
                .borrow_mut()
                .heap
                .init(core::ptr::addr_of_mut!(BYTES).cast(), CAPACITY);
        });
    }
    pub fn seal(&self) {
        critical_section::with(|cs| self.0.borrow(cs).borrow_mut().sealed = true);
    }
    /// Bench-only post-Play preparation; no allocation permission during Play.
    /// SAFETY: the caller must have drained and dropped the sole kernel first.
    #[cfg(feature = "kernel-evaluation")]
    pub unsafe fn prepare_after_drained_play(&self) {
        critical_section::with(|cs| self.0.borrow(cs).borrow_mut().sealed = false);
    }
    pub fn observation(&self) -> (usize, usize, usize) {
        critical_section::with(|cs| {
            let state = self.0.borrow(cs).borrow();
            (state.peak, state.heap.used(), state.allocations)
        })
    }
}

unsafe impl GlobalAlloc for ProbeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        critical_section::with(|cs| {
            let mut state = self.0.borrow(cs).borrow_mut();
            if state.sealed {
                #[cfg(feature = "kernel-evaluation")]
                {
                    crate::kernel_fault::record(
                        2,
                        state.peak,
                        state.heap.used(),
                        layout.size(),
                        layout.align(),
                        0,
                        0,
                    );
                    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
                }
                return core::ptr::null_mut();
            }
            match state.heap.allocate_first_fit(layout) {
                Ok(pointer) => {
                    state.allocations += 1;
                    state.peak = state.peak.max(state.heap.used());
                    pointer.as_ptr()
                }
                Err(_) => {
                    #[cfg(feature = "kernel-evaluation")]
                    {
                        crate::kernel_fault::record(
                            1,
                            state.peak,
                            state.heap.used(),
                            layout.size(),
                            layout.align(),
                            0,
                            0,
                        );
                        embassy_rp::rom_data::reset_to_usb_boot(0, 0);
                    }
                    core::ptr::null_mut()
                }
            }
        })
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        critical_section::with(|cs| unsafe {
            self.0
                .borrow(cs)
                .borrow_mut()
                .heap
                .deallocate(NonNull::new_unchecked(pointer), layout);
        });
    }
}
