//! Physical backing for the unchanged 16KiB construction floor. The intrusive
//! guard keeps ownership inside its allocated chunks, with constant stack use.
//! These bytes confer no execution authority and are never used as workspace.
extern crate alloc;
use core::{alloc::Layout, mem::MaybeUninit, ptr::NonNull};
pub(super) const BLOCK_BYTES: usize = 256;
const BLOCKS: usize = 16 * 1024 / BLOCK_BYTES;
#[repr(C)]
struct Chunk {
    next: Option<NonNull<Chunk>>,
    _unread: [MaybeUninit<u8>; BLOCK_BYTES - core::mem::size_of::<Option<NonNull<Chunk>>>()],
}
const _: () = assert!(core::mem::size_of::<Chunk>() == BLOCK_BYTES);
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
    blocks: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Refused {
    pub(super) backed_bytes: usize,
    pub(super) required_bytes: usize,
}
pub(super) fn reserve<A: Allocator>(allocator: &mut A) -> Result<Floor<'_, A>, Refused> {
    let mut floor = Floor {
        allocator,
        head: None,
        blocks: 0,
    };
    for _ in 0..BLOCKS {
        // SAFETY: a successful allocation has the exact Chunk layout and unique
        // ownership. Only its next link is initialized/read; payload stays unread.
        let raw = unsafe { floor.allocator.allocate(Layout::new::<Chunk>()) };
        let Some(chunk) = NonNull::new(raw.cast::<Chunk>()) else {
            return Err(Refused {
                backed_bytes: floor.blocks * BLOCK_BYTES,
                required_bytes: BLOCKS * BLOCK_BYTES,
            });
        };
        unsafe { core::ptr::addr_of_mut!((*chunk.as_ptr()).next).write(floor.head) };
        floor.head = Some(chunk);
        floor.blocks += 1;
    }
    Ok(floor)
}
impl<A: Allocator> Drop for Floor<'_, A> {
    fn drop(&mut self) {
        // Iterative retirement: neither success nor partial refusal recursively
        // drops a list or materializes an array proportional to the floor size.
        while let Some(chunk) = self.head.take() {
            // SAFETY: each initialized next link is read exactly once before
            // releasing its original unique allocation with the exact layout.
            self.head = unsafe { core::ptr::addr_of!((*chunk.as_ptr()).next).read() };
            unsafe {
                self.allocator
                    .deallocate(chunk.as_ptr().cast(), Layout::new::<Chunk>())
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
    }
    impl<const N: usize> Holes<N> {
        fn new(free: [usize; N]) -> Self {
            Self {
                free,
                live: BTreeMap::new(),
                allocations: 0,
                deallocations: 0,
            }
        }
    }
    // SAFETY: the finite-hole model gates real global allocations. Every unique
    // successful allocation is recorded and returned under its original layout.
    unsafe impl<const N: usize> Allocator for Holes<N> {
        unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
            let Some(hole) = self.free.iter().position(|free| *free >= layout.size()) else {
                return core::ptr::null_mut();
            };
            let raw = unsafe { alloc::alloc::alloc(layout) };
            if !raw.is_null() {
                self.free[hole] -= layout.size();
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
            self.free[hole] += bytes;
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
        assert_eq!(floor.blocks * BLOCK_BYTES, 16384);
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
            let initial = [allowed * BLOCK_BYTES];
            let mut allocator = Holes::new(initial);
            let refusal = reserve(&mut allocator).err().unwrap();
            assert_eq!(refusal.backed_bytes, initial[0]);
            assert_eq!(allocator.free, initial);
            assert!(allocator.live.is_empty());
            assert_eq!(allocator.allocations, allowed);
            assert_eq!(allocator.deallocations, allowed);
        }
    }
}
