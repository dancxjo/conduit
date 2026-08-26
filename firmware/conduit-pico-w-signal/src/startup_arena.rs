//! Finite startup-only allocation arena for the USB remote session identities.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use portable_atomic::{AtomicBool, AtomicUsize, Ordering};

// Admits the network bootstrap identities plus the three R1 recovery sinks.
// The arena is sealed before the first R1 Signal Play becomes active.
const STARTUP_ARENA_BYTES: usize = 24 * 1024;

pub struct StartupArena {
    bytes: UnsafeCell<[u8; STARTUP_ARENA_BYTES]>,
    next: AtomicUsize,
    sealed: AtomicBool,
}

unsafe impl Sync for StartupArena {}

impl StartupArena {
    pub const fn new() -> Self {
        Self {
            bytes: UnsafeCell::new([0; STARTUP_ARENA_BYTES]),
            next: AtomicUsize::new(0),
            sealed: AtomicBool::new(false),
        }
    }

    #[cfg(any(feature = "wifi-bootstrap", feature = "bluetooth-line"))]
    pub fn seal(&self) {
        self.sealed.store(true, Ordering::Release);
    }
}

unsafe impl GlobalAlloc for StartupArena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.sealed.load(Ordering::Acquire) {
            #[cfg(feature = "wifi-bootstrap")]
            crate::panic_recovery::record_post_play_start_allocation();
            return core::ptr::null_mut();
        }
        let base = self.bytes.get().cast::<u8>() as usize;
        let mut current = self.next.load(Ordering::Relaxed);
        loop {
            let Some(aligned_address) = (base + current)
                .checked_add(layout.align() - 1)
                .map(|address| address & !(layout.align() - 1))
            else {
                return core::ptr::null_mut();
            };
            let aligned_offset = aligned_address - base;
            let Some(next) = aligned_offset.checked_add(layout.size()) else {
                return core::ptr::null_mut();
            };
            if next > STARTUP_ARENA_BYTES {
                return core::ptr::null_mut();
            }
            match self
                .next
                .compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return aligned_address as *mut u8,
                Err(observed) => current = observed,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
