//! Separate unique uninitialized kernel storage from later Sign backing.
use super::*;
/// Kernel storage only: no initialized access, Source, grant or provider owner.
/// Keeping this largest block early protects its allocation geometry while
/// smaller temporary preparation storage is separately admitted and retired.
pub struct PreparedNetworkKernelBacking {
    backing: alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>,
}
/// Complete execution storage without execution authority or initialized kernel.
pub struct PreparedNetworkKernelStorage {
    backing: PreparedNetworkKernelBacking,
    signs: k::HostedSignLog,
}
/// Allocation observations only; never capacity or authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKernelAllocationPhase {
    Kernel,
    Signs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkKernelAllocationFailure {
    pub phase: NetworkKernelAllocationPhase,
    pub requested_bytes: usize,
    pub alignment: usize,
}
impl PreparedNetworkKernelBacking {
    pub const fn allocation_bytes() -> usize {
        core::mem::size_of::<PreparedNetworkKernel>()
    }
    pub fn try_new() -> Result<Self, ControlKernelRefusal> {
        Self::try_new_observing_failure(|_| ())
    }
    pub fn try_new_observing_failure(
        observe: impl FnOnce(NetworkKernelAllocationFailure),
    ) -> Result<Self, ControlKernelRefusal> {
        Self::allocate().map_err(|failure| {
            observe(failure);
            ControlKernelRefusal::Storage
        })
    }
    fn allocate() -> Result<Self, NetworkKernelAllocationFailure> {
        let layout = core::alloc::Layout::new::<PreparedNetworkKernel>();
        // SAFETY: exact unique nonzero uninitialized backing, initialized only
        // by complete storage's successful Box::write after all preparation.
        let raw = unsafe { alloc::alloc::alloc(layout) }
            .cast::<core::mem::MaybeUninit<PreparedNetworkKernel>>();
        if raw.is_null() {
            return Err(NetworkKernelAllocationFailure {
                phase: NetworkKernelAllocationPhase::Kernel,
                requested_bytes: layout.size(),
                alignment: layout.align(),
            });
        }
        // SAFETY: this is the same successful unique allocation and Layout.
        Ok(Self {
            backing: unsafe { alloc::boxed::Box::from_raw(raw) },
        })
    }
    pub fn as_ptr(&self) -> *const PreparedNetworkKernel {
        self.backing.as_ptr()
    }
    /// Allocate only Signs after temporary descriptions retire. Refusal returns
    /// the exact original unique backing, never a reconstructed address/owner.
    pub fn with_signs_observing_failure(
        self,
        observe: impl FnOnce(NetworkKernelAllocationFailure),
    ) -> Result<PreparedNetworkKernelStorage, (Self, ControlKernelRefusal)> {
        let signs = match k::HostedSignLog::try_new(
            128,
            (128 * core::mem::size_of::<k::KernelEvent>()) as u32,
        ) {
            Ok(signs) => signs,
            Err(k::HostedSignPreparationError::Allocation) => {
                observe(NetworkKernelAllocationFailure {
                    phase: NetworkKernelAllocationPhase::Signs,
                    requested_bytes: CONTROL_SIGN_BACKING_BYTES,
                    alignment: k::HostedSignLog::canonical_local_storage_alignment(),
                });
                return Err((self, ControlKernelRefusal::Storage));
            }
            Err(k::HostedSignPreparationError::Budget(_)) => {
                return Err((self, ControlKernelRefusal::Storage))
            }
        };
        Ok(PreparedNetworkKernelStorage {
            backing: self,
            signs,
        })
    }
}
impl PreparedNetworkKernelStorage {
    pub const fn allocation_bytes() -> usize {
        PreparedNetworkKernelBacking::allocation_bytes() + CONTROL_SIGN_BACKING_BYTES
    }
    pub fn try_new() -> Result<Self, ControlKernelRefusal> {
        Self::try_new_observing_failure(|_| ())
    }
    /// Observe allocation refusal only; preserve the previous complete-storage
    /// constructor's behavior by explicitly retiring unused backing on refusal.
    pub fn try_new_observing_failure(
        observe: impl FnOnce(NetworkKernelAllocationFailure),
    ) -> Result<Self, ControlKernelRefusal> {
        let backing = match PreparedNetworkKernelBacking::allocate() {
            Ok(backing) => backing,
            Err(failure) => {
                observe(failure);
                return Err(ControlKernelRefusal::Storage);
            }
        };
        backing
            .with_signs_observing_failure(observe)
            .map_err(|(backing, error)| {
                drop(backing);
                error
            })
    }
    pub fn as_ptr(&self) -> *const PreparedNetworkKernel {
        self.backing.as_ptr()
    }
    #[inline(never)]
    pub fn finish(
        self,
        parts: PreparedNetworkKernelParts,
    ) -> Result<alloc::boxed::Box<PreparedNetworkKernel>, ControlKernelRefusal> {
        let kernel = parts.finish_with_signs(self.signs)?;
        Ok(alloc::boxed::Box::write(self.backing.backing, kernel))
    }
}
