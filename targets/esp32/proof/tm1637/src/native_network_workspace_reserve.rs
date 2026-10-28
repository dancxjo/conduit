//! Physical backing for the unchanged 16KiB construction floor. Several
//! allocations can occupy smaller free regions without stranding the final
//! kernel's contiguous backing. These blocks confer no execution authority.
pub(super) const BLOCK_BYTES: usize = 2048;
const BLOCKS: usize = 8;
pub(super) fn reserve<T, E>(
    mut allocate: impl FnMut(usize) -> Result<T, E>,
) -> Result<[T; BLOCKS], E> {
    let mut blocks = core::array::from_fn::<_, BLOCKS, _>(|_| None);
    for block in &mut blocks {
        *block = Some(allocate(BLOCK_BYTES)?);
    }
    Ok(blocks.map(|block| block.expect("all floor blocks allocated")))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    // Finite first-fit holes matching observed free bank totals.
    // This models the allocation constraint, not a target allocator proof.
    struct Block {
        holes: Rc<RefCell<[usize; 4]>>,
        hole: usize,
        bytes: usize,
    }
    impl Drop for Block {
        fn drop(&mut self) {
            self.holes.borrow_mut()[self.hole] += self.bytes;
        }
    }
    fn allocate(holes: &Rc<RefCell<[usize; 4]>>, bytes: usize) -> Result<Block, ()> {
        let hole = holes
            .borrow()
            .iter()
            .position(|free| *free >= bytes)
            .ok_or(())?;
        holes.borrow_mut()[hole] -= bytes;
        Ok(Block {
            holes: holes.clone(),
            hole,
            bytes,
        })
    }
    #[test]
    fn same_floor_preserves_large_kernel_hole_across_fragmented_banks() {
        let initial = [2580, 2224, 22052, 0];
        // The second allocation represents concurrent construction pressure;
        // its size is a counterexample, not a measured target owner size.
        let holes = Rc::new(RefCell::new(initial));
        let contiguous_floor = allocate(&holes, 16384).unwrap();
        let owner = allocate(&holes, 3288).unwrap();
        assert!(allocate(&holes, 3288).is_err());
        drop(owner);
        drop(contiguous_floor);
        assert_eq!(*holes.borrow(), initial);
        let floor = reserve(|bytes| allocate(&holes, bytes)).unwrap();
        assert_eq!(
            initial.iter().sum::<usize>() - holes.borrow().iter().sum::<usize>(),
            16384
        );
        let owner = allocate(&holes, 3288).unwrap();
        let kernel = allocate(&holes, 3288).unwrap();
        drop(kernel);
        drop(owner);
        drop(floor);
        assert_eq!(*holes.borrow(), initial);
    }
    #[test]
    fn partial_allocation_refusal_returns_every_previous_block() {
        let initial = [2048, 2048, 2048, 2048];
        let holes = Rc::new(RefCell::new(initial));
        assert!(reserve(|bytes| allocate(&holes, bytes)).is_err());
        assert_eq!(*holes.borrow(), initial);
    }
}
