//! One boot-scoped admitted allocator used only before Play start.

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct BootArena {
    start: AtomicUsize,
    end: AtomicUsize,
    next: AtomicUsize,
    sealed: AtomicBool,
}

impl BootArena {
    pub const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
            sealed: AtomicBool::new(false),
        }
    }

    /// # Safety
    ///
    /// `start..start + length` must be one exclusively owned writable virtual
    /// range for the duration of this boot.
    pub unsafe fn initialize(&self, start: usize, length: usize) -> Result<(), ArenaError> {
        let end = start.checked_add(length).ok_or(ArenaError::InvalidRange)?;
        if start == 0 || length == 0 || self.start.swap(start, Ordering::SeqCst) != 0 {
            return Err(ArenaError::InvalidRange);
        }
        self.end.store(end, Ordering::SeqCst);
        self.next.store(start, Ordering::SeqCst);
        Ok(())
    }

    pub fn seal(&self) -> usize {
        self.sealed.store(true, Ordering::SeqCst);
        self.used()
    }

    pub fn used(&self) -> usize {
        self.next
            .load(Ordering::SeqCst)
            .saturating_sub(self.start.load(Ordering::SeqCst))
    }

    pub fn capacity(&self) -> usize {
        self.end
            .load(Ordering::SeqCst)
            .saturating_sub(self.start.load(Ordering::SeqCst))
    }
}

impl Default for BootArena {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaError {
    InvalidRange,
}

unsafe impl GlobalAlloc for BootArena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.sealed.load(Ordering::SeqCst) {
            return null_mut();
        }
        let end = self.end.load(Ordering::SeqCst);
        let mut current = self.next.load(Ordering::SeqCst);
        loop {
            let Some(aligned) = current
                .checked_add(layout.align() - 1)
                .map(|value| value & !(layout.align() - 1))
            else {
                return null_mut();
            };
            let Some(next) = aligned.checked_add(layout.size()) else {
                return null_mut();
            };
            if next > end {
                return null_mut();
            }
            match self
                .next
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return aligned as *mut u8,
                Err(observed) => current = observed,
            }
        }
    }

    unsafe fn dealloc(&self, _pointer: *mut u8, _layout: Layout) {}
}

#[cfg_attr(target_os = "none", global_allocator)]
pub static BOOT_ARENA: BootArena = BootArena::new();
