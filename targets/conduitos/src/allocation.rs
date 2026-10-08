//! Fixed-capacity storage for Boot-scoped preparation.
//!
//! Dropping a prepared object returns its storage to this same admitted range.
//! Successive graphical Presentations therefore reuse storage rather than
//! consuming a new lifetime allocation on every refresh. Sealing still refuses
//! all subsequent allocations; the kernel's allocation-free proof is unchanged.
//! Interrupt handlers must not allocate or enter this preparation allocator.

mod bitmap;

use bitmap::ArenaState;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    hint::spin_loop,
    sync::atomic::{AtomicBool, Ordering},
};

/// Maximum finite preparation range; Root selects its exact Make budget.
pub const MAXIMUM_ARENA_BYTES: usize = 256 * 1024 * 1024;

pub struct BootArena {
    locked: AtomicBool,
    state: UnsafeCell<ArenaState>,
    requests: UnsafeCell<AllocationRequests>,
}

// The lock exclusively owns every access to state. Allocated ranges are
// disjoint; their access and deallocation obey GlobalAlloc's caller contract.
unsafe impl Sync for BootArena {}

impl BootArena {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            state: UnsafeCell::new(ArenaState::new()),
            requests: UnsafeCell::new(AllocationRequests {
                total: 0,
                after_seal: 0,
            }),
        }
    }

    /// # Safety
    /// `start..start + length` must be one exclusively owned writable virtual
    /// range for this arena's lifetime, without overlapping the arena metadata.
    pub unsafe fn initialize(&self, start: usize, length: usize) -> Result<(), ArenaError> {
        self.with_state(|state| state.initialize(start, length))
    }

    pub fn seal(&self) -> usize {
        self.with_state(|state| {
            state.sealed = true;
            state.used()
        })
    }

    /// Peak live storage for the existing before/after-seal proof receipt.
    pub fn used(&self) -> usize {
        self.with_state(|state| state.used())
    }

    /// Currently live storage, including fixed allocation-unit rounding.
    pub fn live_bytes(&self) -> usize {
        self.with_state(|state| state.live_bytes())
    }

    pub fn capacity(&self) -> usize {
        self.with_state(|state| state.capacity())
    }

    /// Counts calls to alloc/realloc, including refused requests. Saturation
    /// must be treated as an invalid no-allocation receipt by the caller.
    pub fn allocation_requests(&self) -> AllocationRequests {
        self.with_state(|_| unsafe { *self.requests.get() })
    }

    fn record_request(&self, sealed: bool) {
        // SAFETY: only called while with_state exclusively owns the lock.
        let requests = unsafe { &mut *self.requests.get() };
        requests.total = requests.total.saturating_add(1);
        if sealed {
            requests.after_seal = requests.after_seal.saturating_add(1);
        }
    }

    fn with_state<R>(&self, work: impl FnOnce(&mut ArenaState) -> R) -> R {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        struct Unlock<'a>(&'a AtomicBool);
        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _unlock = Unlock(&self.locked);
        // SAFETY: this guard exclusively owns state until work returns. Work
        // never recursively allocates, and no reference to state is returned.
        work(unsafe { &mut *self.state.get() })
    }
}

impl Default for BootArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Diagnostic request counts; deallocation does not count as allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationRequests {
    pub total: usize,
    pub after_seal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaError {
    InvalidRange,
}

unsafe impl GlobalAlloc for BootArena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.with_state(|state| {
            self.record_request(state.sealed);
            state.allocate(layout)
        })
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        self.with_state(|state| state.release(pointer, layout));
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: GlobalAlloc supplies the exact live allocation and Layout.
        self.with_state(|state| {
            self.record_request(state.sealed);
            unsafe { state.resize(pointer, layout, new_size) }
        })
    }
}

#[cfg_attr(target_os = "none", global_allocator)]
pub static BOOT_ARENA: BootArena = BootArena::new();

#[cfg(test)]
mod tests;
