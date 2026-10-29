use alloc::vec::Vec;

/// Allocation-free view of one collection's inline storage.
///
/// Element size excludes allocations owned by an element (such as identity
/// strings). Capacity describes retained storage, not an admission grant.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VecStorageUsage {
    pub len: usize,
    pub capacity: usize,
    pub element_bytes: usize,
}

impl VecStorageUsage {
    pub(crate) fn inspect<T>(collection: &Vec<T>) -> Self {
        Self {
            len: collection.len(),
            capacity: collection.capacity(),
            element_bytes: core::mem::size_of::<T>(),
        }
    }
}
