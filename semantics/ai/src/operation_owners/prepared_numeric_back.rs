//! Retained concrete numeric Back inventory before trait-object erasure.
//! Does not reserve construction work or charge shared model/tensor owners,
//! allocator bookkeeping, the returned inline wrapper, or stack.
use alloc::boxed::Box;
use conduit_kernel::scheduler::StepBack;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericBackStorage {
    concrete_root_bytes: usize,
    local_accounted_heap_bytes: usize,
}
impl NumericBackStorage {
    pub fn concrete_root_bytes(self) -> usize {
        self.concrete_root_bytes
    }
    pub fn local_accounted_heap_bytes(self) -> usize {
        self.local_accounted_heap_bytes
    }
    /// None means overflow; it must never be treated as an admitted charge.
    pub fn accounted_retained_bytes(self) -> Option<usize> {
        self.concrete_root_bytes
            .checked_add(self.local_accounted_heap_bytes)
    }
}
pub struct PreparedNumericBack {
    back: Box<dyn StepBack<PORTS> + Send>,
    storage: NumericBackStorage,
}
impl PreparedNumericBack {
    pub(super) fn new<T: StepBack<PORTS> + Send + 'static>(back: T, local: usize) -> Self {
        Self {
            back: Box::new(back),
            storage: NumericBackStorage {
                concrete_root_bytes: core::mem::size_of::<T>(),
                local_accounted_heap_bytes: local,
            },
        }
    }
    pub fn storage(&self) -> NumericBackStorage {
        self.storage
    }
    pub fn into_back(self) -> Box<dyn StepBack<PORTS> + Send> {
        self.back
    }
}
