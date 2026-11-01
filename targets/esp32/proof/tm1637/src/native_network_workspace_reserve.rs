//! Physical backing for the unchanged 16KiB construction floor. The intrusive
//! guard keeps ownership inside its allocated chunks, with constant stack use.
//! These bytes confer no execution authority and are never used as workspace.
extern crate alloc;
use core::{alloc::Layout, ptr::NonNull};
pub(super) const BLOCK_BYTES: usize = 256;
const FLOOR_BYTES: usize = 16 * 1024;
const BLOCKS: usize = FLOOR_BYTES / BLOCK_BYTES;
#[repr(C)]
struct Chunk {
    next: Option<NonNull<Chunk>>,
    bytes: usize,
}
const _: () = assert!(core::mem::size_of::<Chunk>() <= BLOCK_BYTES);
const _: () = assert!(BLOCK_BYTES * BLOCKS == 16 * 1024);

/// # Safety
/// Successful allocations must be unique, writable and valid for the supplied
/// layout until deallocation. Deallocation must accept exactly those layouts.
pub(super) unsafe trait Allocator {
    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8;
    unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout);
}
pub(super) struct Global;
// SAFETY: the global allocator implements these layout/ownership obligations.
unsafe impl Allocator for Global {
    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        unsafe { alloc::alloc::alloc(layout) }
    }
    unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout) {
        unsafe { alloc::alloc::dealloc(pointer, layout) }
    }
}
pub(super) struct Floor<'a, A: Allocator> {
    allocator: &'a mut A,
    head: Option<NonNull<Chunk>>,
    backed_bytes: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Refused {
    pub(super) backed_bytes: usize,
    pub(super) required_bytes: usize,
}
fn chunk_layout(bytes: usize) -> Layout {
    // Every caller uses a power of two from256 through16384, with this header's
    // alignment. The minimum includes the header; no payload byte is read.
    Layout::from_size_align(bytes, core::mem::align_of::<Chunk>()).unwrap()
}
pub(super) fn reserve<A: Allocator>(allocator: &mut A) -> Result<Floor<'_, A>, Refused> {
    let mut floor = Floor {
        allocator,
        head: None,
        backed_bytes: 0,
    };
    while floor.backed_bytes < FLOOR_BYTES {
        let remaining = FLOOR_BYTES - floor.backed_bytes;
        let mut bytes = 1usize << (usize::BITS - 1 - remaining.leading_zeros());
        loop {
            // Large chunks avoid per-allocation overhead when contiguous backing
            // exists; refusal halves down to the unchanged256-byte minimum.
            let raw = unsafe { floor.allocator.allocate(chunk_layout(bytes)) };
            if let Some(chunk) = NonNull::new(raw.cast::<Chunk>()) {
                // SAFETY: the exact allocation contains this header. Only its
                // initialized link and original layout size are ever read.
                unsafe {
                    chunk.as_ptr().write(Chunk {
                        next: floor.head,
                        bytes,
                    })
                };
                floor.head = Some(chunk);
                floor.backed_bytes += bytes;
                break;
            }
            if bytes == BLOCK_BYTES {
                return Err(Refused {
                    backed_bytes: floor.backed_bytes,
                    required_bytes: FLOOR_BYTES,
                });
            }
            bytes /= 2;
        }
    }
    // At least256 bytes per success: <=64 successes and <=64*7 attempts.
    Ok(floor)
}

impl<A: Allocator> Drop for Floor<'_, A> {
    fn drop(&mut self) {
        // Iterative retirement: neither success nor partial refusal recursively
        // drops a list or materializes an array proportional to the floor size.
        while let Some(chunk) = self.head.take() {
            // SAFETY: each initialized next link is read exactly once before
            // releasing its original unique allocation with the exact layout.
            let Chunk { next, bytes } = unsafe { chunk.as_ptr().read() };
            self.head = next;
            unsafe {
                self.allocator
                    .deallocate(chunk.as_ptr().cast(), chunk_layout(bytes))
            };
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Holes<const N: usize> {
        free: [usize; N],
        live: BTreeMap<usize, (usize, usize)>,
        allocations: usize,
        deallocations: usize,
        attempts: usize,
        overhead: usize,
    }
    impl<const N: usize> Holes<N> {
        fn new(free: [usize; N]) -> Self {
            Self {
                free,
                live: BTreeMap::new(),
                allocations: 0,
                deallocations: 0,
                attempts: 0,
                overhead: 0,
            }
        }
    }
    // SAFETY: the finite-hole model gates real global allocations. Every unique
    // successful allocation is recorded and returned under its original layout.
    unsafe impl<const N: usize> Allocator for Holes<N> {
        unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
            self.attempts += 1;
            let charged = layout.size() + self.overhead;
            let Some(hole) = self.free.iter().position(|free| *free >= charged) else {
                return core::ptr::null_mut();
            };
            let raw = unsafe { alloc::alloc::alloc(layout) };
            if !raw.is_null() {
                self.free[hole] -= charged;
                assert!(self
                    .live
                    .insert(raw as usize, (hole, layout.size()))
                    .is_none());
                self.allocations += 1;
            }
            raw
        }
        unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout) {
            let (hole, bytes) = self.live.remove(&(pointer as usize)).unwrap();
            assert_eq!(bytes, layout.size());
            self.free[hole] += bytes + self.overhead;
            self.deallocations += 1;
            unsafe { alloc::alloc::dealloc(pointer, layout) };
        }
    }
    #[test]
    fn same_floor_backs_sub_512_byte_holes_and_retires_every_chunk() {
        // Counterexample to a fixed512-byte quantum, not a target hole trace.
        let initial = [384; 64];
        let mut allocator = Holes::new(initial);
        let old = Layout::from_size_align(512, core::mem::align_of::<Chunk>()).unwrap();
        assert!(unsafe { allocator.allocate(old) }.is_null());
        let floor = reserve(&mut allocator).unwrap();
        assert_eq!(floor.backed_bytes, 16384);
        assert_eq!(floor.allocator.free.iter().sum::<usize>(), 8192);
        drop(floor);
        assert_eq!(allocator.free, initial);
        assert!(allocator.live.is_empty());
        assert_eq!(allocator.allocations, BLOCKS);
        assert_eq!(allocator.deallocations, BLOCKS);
    }
    #[test]
    fn partial_refusal_returns_every_chunk_and_reports_actual_shortage() {
        let initial = [2048; 4];
        let mut allocator = Holes::new(initial);
        let refused = reserve(&mut allocator).err().unwrap();
        assert_eq!(
            refused,
            Refused {
                backed_bytes: 8192,
                required_bytes: 16384
            }
        );
        assert_eq!(allocator.free, initial);
        assert!(allocator.live.is_empty());
        assert_eq!(allocator.allocations, allocator.deallocations);
    }
    #[test]
    fn guard_storage_does_not_grow_with_number_of_chunks() {
        assert_eq!(
            core::mem::size_of::<Floor<'_, Global>>(),
            3 * core::mem::size_of::<usize>()
        );
    }
    #[test]
    fn every_allocation_refusal_returns_all_prior_backing() {
        for allowed in 0..BLOCKS {
            let mut initial = [0; BLOCKS];
            initial[..allowed].fill(BLOCK_BYTES);
            let mut allocator = Holes::new(initial);
            let refusal = reserve(&mut allocator).err().unwrap();
            assert_eq!(refusal.backed_bytes, allowed * BLOCK_BYTES);
            assert_eq!(allocator.free, initial);
            assert!(allocator.live.is_empty());
            assert_eq!(allocator.allocations, allowed);
            assert_eq!(allocator.deallocations, allowed);
            assert!(allocator.attempts <= BLOCKS * 7);
        }
    }
    #[test]
    fn mixed_chunk_layouts_release_their_exact_original_allocations() {
        let initial = [8192, 4096, 2048, 1024, 512, 256, 256];
        let mut allocator = Holes::new(initial);
        let floor = reserve(&mut allocator).unwrap();
        assert_eq!(floor.backed_bytes, FLOOR_BYTES);
        assert_eq!(floor.allocator.allocations, initial.len());
        assert!(floor.allocator.attempts <= BLOCKS * 7);
        let mut actual: Vec<_> = floor
            .allocator
            .live
            .values()
            .map(|(_, bytes)| *bytes)
            .collect();
        actual.sort_unstable();
        assert_eq!(actual, [256, 256, 512, 1024, 2048, 4096, 8192]);
        drop(floor);
        assert_eq!(allocator.free, initial);
        assert!(allocator.live.is_empty());
        assert_eq!(allocator.allocations, allocator.deallocations);
    }
    #[test]
    fn larger_chunks_avoid_modeled_per_allocation_metadata_cost() {
        // Deliberate overhead model, not an ESP allocator measurement. Identical
        // byte capacity cannot cover64 headers but can back the same16KiB once.
        let initial = [FLOOR_BYTES + 256];
        let mut allocator = Holes::new(initial);
        allocator.overhead = 16;
        let mut small = Vec::new();
        for _ in 0..BLOCKS {
            let raw = unsafe { allocator.allocate(chunk_layout(BLOCK_BYTES)) };
            if raw.is_null() {
                break;
            }
            small.push(raw);
        }
        assert!(small.len() * BLOCK_BYTES < FLOOR_BYTES);
        for raw in small {
            unsafe { allocator.deallocate(raw, chunk_layout(BLOCK_BYTES)) };
        }
        assert_eq!(allocator.free, initial);
        let before = allocator.allocations;
        let floor = reserve(&mut allocator).unwrap();
        assert_eq!(floor.backed_bytes, FLOOR_BYTES);
        assert_eq!(floor.allocator.allocations - before, 1);
        drop(floor);
        assert_eq!(allocator.free, initial);
        assert!(allocator.live.is_empty());
        assert_eq!(allocator.allocations, allocator.deallocations);
    }
}
