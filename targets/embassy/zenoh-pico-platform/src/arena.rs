//! C allocation boundary over the same allocator used by the Pico firmware.
//! Backing is supplied explicitly; there is no system/global fallback or grow.
use core::{
    alloc::Layout,
    mem::{align_of, size_of, MaybeUninit},
    ptr::{self, NonNull},
};
use linked_list_allocator::Heap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaError {
    Alignment,
    TooSmall,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Allocator-region accounting, not total target RAM. The arena descriptor,
/// worker stack, socket backing and other static memory are additional costs.
pub struct ArenaStats {
    pub capacity: usize,
    pub used: usize,
    pub peak_used: usize,
    pub requested: usize,
    pub peak_requested: usize,
    pub allocations: usize,
    pub peak_allocations: usize,
}
/// One finite caller-provided arena. ALIGN must cover the actual C target's
/// max_align_t, verified by its build; it is not guessed from Rust pointer size.
/// The ALIGN-byte prefix retains the requested size and preserves that alignment.
/// Caller serializes mutable access, normally on the one blocking C worker.
/// Constructing this value does not admit backing, a worker or foreign effects.
pub struct CArena<const ALIGN: usize> {
    heap: Heap,
    requested: usize,
    peak_requested: usize,
    peak_used: usize,
    allocations: usize,
    peak_allocations: usize,
}
impl<const ALIGN: usize> CArena<ALIGN> {
    pub fn new(storage: &'static mut [MaybeUninit<u8>]) -> Result<Self, ArenaError> {
        if !ALIGN.is_power_of_two() || ALIGN < size_of::<usize>() || ALIGN < align_of::<usize>() {
            return Err(ArenaError::Alignment);
        }
        // Heap metadata takes at most three machine words for arbitrary input
        // alignment. Enforce this before using its safe static-slice entrance.
        if storage.len() < 3 * size_of::<usize>() {
            return Err(ArenaError::TooSmall);
        }
        Ok(Self {
            heap: Heap::from_slice(storage),
            requested: 0,
            peak_requested: 0,
            peak_used: 0,
            allocations: 0,
            peak_allocations: 0,
        })
    }
    fn layout(size: usize) -> Option<Layout> {
        Layout::from_size_align(size.checked_add(ALIGN)?, ALIGN).ok()
    }
    /// Zero-sized requests return NULL. Exhaustion/overflow returns NULL without
    /// growing the arena, falling back, or consuming another resource grant.
    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        if size == 0 {
            return ptr::null_mut();
        }
        let Some(layout) = Self::layout(size) else {
            return ptr::null_mut();
        };
        let Ok(base) = self.heap.allocate_first_fit(layout) else {
            return ptr::null_mut();
        };
        self.requested += size;
        self.allocations += 1;
        self.peak_requested = self.peak_requested.max(self.requested);
        self.peak_allocations = self.peak_allocations.max(self.allocations);
        self.peak_used = self.peak_used.max(self.heap.used());
        // SAFETY: the allocated prefix is at least usize-sized and aligned;
        // adding ALIGN keeps the returned payload aligned within this block.
        unsafe {
            base.as_ptr().cast::<usize>().write(size);
            base.as_ptr().add(ALIGN)
        }
    }
    /// NULL is valid and does nothing. Payload and size prefix are wiped before
    /// returning the block to the finite allocator.
    ///
    /// # Safety
    /// `pointer` is NULL or a live pointer from this arena, freed exactly once.
    /// Its prefix is intact and no caller uses the allocation after this call.
    pub unsafe fn free(&mut self, pointer: *mut u8) {
        if pointer.is_null() {
            return;
        }
        // SAFETY: caller guarantees an intact live allocation from this arena.
        unsafe {
            let base = pointer.sub(ALIGN);
            let size = base.cast::<usize>().read();
            let layout = Self::layout(size).expect("live C allocation layout");
            self.requested -= size;
            self.allocations -= 1;
            base.write_bytes(0, layout.size());
            self.heap.deallocate(NonNull::new_unchecked(base), layout);
        }
    }
    /// NULL behaves as allocate; size zero frees and returns NULL. Failure
    /// preserves the original allocation and bytes. Growth/shrink allocates a
    /// replacement before releasing the old block, so peaks include overlap.
    ///
    /// # Safety
    /// Same live-pointer/prefix/exclusive-use requirements as `free`. On success
    /// the original pointer is retired; on failure it remains the caller's.
    pub unsafe fn reallocate(&mut self, pointer: *mut u8, size: usize) -> *mut u8 {
        if size == 0 {
            // SAFETY: forwarded caller ownership requirements.
            unsafe {
                self.free(pointer);
            }
            return ptr::null_mut();
        }
        if pointer.is_null() {
            return self.allocate(size);
        }
        let replacement = self.allocate(size);
        if replacement.is_null() {
            return replacement;
        }
        // SAFETY: disjoint live blocks and an intact old prefix; raw byte copy
        // preserves only the prefix contained in both allocation extents.
        unsafe {
            let old_size = pointer.sub(ALIGN).cast::<usize>().read();
            ptr::copy_nonoverlapping(pointer, replacement, old_size.min(size));
            self.free(pointer);
        }
        replacement
    }
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            capacity: self.heap.size(),
            used: self.heap.used(),
            peak_used: self.peak_used,
            requested: self.requested,
            peak_requested: self.peak_requested,
            allocations: self.allocations,
            peak_allocations: self.peak_allocations,
        }
    }
}
