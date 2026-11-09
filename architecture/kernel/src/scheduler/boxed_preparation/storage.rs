//! Affine physical storage before provider transfer; no execution authority.
use super::SchedulerError;
use alloc::boxed::Box;
use core::{alloc::Layout, mem::MaybeUninit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelPreparationError {
    Topology(SchedulerError),
    Capacity { required: usize, available: usize },
    Allocation,
}
impl From<SchedulerError> for KernelPreparationError {
    fn from(value: SchedulerError) -> Self {
        Self::Topology(value)
    }
}
impl From<crate::ProtocolError> for KernelPreparationError {
    fn from(value: crate::ProtocolError) -> Self {
        Self::Topology(value.into())
    }
}

pub(super) fn allocate<K>(
    maximum_bytes: usize,
) -> Result<Box<MaybeUninit<K>>, KernelPreparationError> {
    let layout = Layout::new::<K>();
    if layout.size() > maximum_bytes {
        return Err(KernelPreparationError::Capacity {
            required: layout.size(),
            available: maximum_bytes,
        });
    }
    // Only instantiated for a scheduler, whose counters make its size nonzero.
    assert_ne!(layout.size(), 0);
    // SAFETY: a nonzero valid layout; null is handled without aborting or forming
    // a Box. MaybeUninit has exactly K's allocation layout and drops no K fields.
    let pointer = unsafe { alloc::alloc::alloc(layout) };
    if pointer.is_null() {
        return Err(KernelPreparationError::Allocation);
    }
    Ok(unsafe { Box::from_raw(pointer.cast::<MaybeUninit<K>>()) })
}

/// Checked numeric topology and acquired kernel storage, awaiting actual Backs.
/// This supplies no Plan approval, resource reservation or permission to start.
/// Dropping releases initialized stores and allocation without touching Backs.
pub struct PreparedKernelStorage<D, K, const N: usize> {
    storage: Option<Box<MaybeUninit<K>>>,
    drop_initialized: unsafe fn(*mut K),
    bind: unsafe fn(*mut K, [D; N]),
    // Private construction pairs D with K's driver field. Prevent independent
    // lifetime coercions from changing that pairing before the unsafe writer.
    driver_invariant: core::marker::PhantomData<fn(D) -> D>,
    kernel_invariant: core::marker::PhantomData<fn(K) -> K>,
}
impl<D, K, const N: usize> PreparedKernelStorage<D, K, N> {
    /// Private construction requires exact-layout owned storage, all K fields
    /// initialized except its drivers, and matching exhaustive cleanup/binders.
    pub(super) fn new(
        storage: Box<MaybeUninit<K>>,
        drop_initialized: unsafe fn(*mut K),
        bind: unsafe fn(*mut K, [D; N]),
    ) -> Self {
        Self {
            storage: Some(storage),
            drop_initialized,
            bind,
            driver_invariant: core::marker::PhantomData,
            kernel_invariant: core::marker::PhantomData,
        }
    }

    pub fn storage_bytes(&self) -> usize {
        core::mem::size_of::<K>()
    }
    /// Allocation identity only; K is not yet fully initialized or executable.
    pub fn storage_address(&self) -> *const () {
        self.storage
            .as_ref()
            .expect("live prepared storage")
            .as_ptr()
            .cast()
    }

    /// Infallible and allocation-free. Caller first admits the whole workset and
    /// supplies the exact independently checked, actually possessed Back owners.
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    pub fn bind(mut self, drivers: [D; N]) -> Box<K> {
        let mut storage = self.storage.take().expect("live prepared storage");
        // SAFETY: the private constructor pairs this storage with its exact
        // driver writer. This initializes the final field exactly once.
        unsafe {
            (self.bind)(storage.as_mut_ptr(), drivers);
            storage.assume_init()
        }
    }
}
impl<D, K, const N: usize> Drop for PreparedKernelStorage<D, K, N> {
    fn drop(&mut self) {
        if let Some(mut storage) = self.storage.take() {
            // SAFETY: only the private constructor supplies this exhaustive
            // destructor for initialized fields. Drivers have never been bound.
            unsafe {
                (self.drop_initialized)(storage.as_mut_ptr());
            }
            // Dropping Box<MaybeUninit<K>> deallocates without dropping K again.
        }
    }
}
