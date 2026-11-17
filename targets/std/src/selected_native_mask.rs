//! Actual fixed native owners initialized on the original IMAGE counter.
//! This does not admit a per-Plan Mask, manufacture a Show or authorize input.
mod fixed;
mod images;
mod initialization;
use crate::{
    kernel_preparation::{BaseBackingReservation, ImageMemoryLease},
    StdHost,
};
use conduit_conduitos_mask_offer::NativeMaskStorageProfile;
use conduit_core::*;
use conduitos::{
    native_compositor::PreparedCompositorBacking,
    product_bases::{
        NativeMaskBackingReceipt, NativeSurfaceProvider, RetainedNativeMaskStorage,
        UnboundNativeMaskStorage,
    },
};
pub use images::{
    NativeMaskImageBackingReceipt, NativeMaskImageData, NativeMaskImageInitialization,
    NativeMaskImagePlane, NATIVE_MASK_IMAGE_PREPARATION_BYTES, NATIVE_MASK_IMAGE_RETAINED_BYTES,
};
pub(crate) use initialization::InitialNativeMask;
pub(crate) const SURFACE: &str = "selected/native-mask";
/// Fixed actual stores; excludes SDL/GPU/RSS and unqualified per-Plan work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMaskFixedBackingReceipt {
    pub mask: NativeMaskBackingReceipt,
    pub compositor_bytes: usize,
    pub scanout_cpu_bytes: usize,
    pub runtime_pool_bytes: usize,
    pub image_heap_charge: usize,
}
pub struct SelectedNativeMaskBase {
    // Stores and image data retire before the matching original memory guards.
    _kernel: UnboundNativeMaskStorage,
    _raster: PreparedCompositorBacking,
    input: Option<conduit_linux_display::NativeInputHostClaim>,
    images: Option<images::NativeMaskImages>,
    _provider: NativeSurfaceProvider,
    entry: BaseProviderEntry,
    _backing: BaseBackingReservation,
    receipt: NativeMaskFixedBackingReceipt,
    _heap: ImageMemoryLease,
}
impl SelectedNativeMaskBase {
    pub fn fixed_backing(&self) -> NativeMaskFixedBackingReceipt {
        self.receipt
    }
    pub fn private_image_backing(&self) -> Option<NativeMaskImageBackingReceipt> {
        self.images.as_ref().map(images::NativeMaskImages::receipt)
    }
    pub fn entry(&self) -> &BaseProviderEntry {
        &self.entry
    }
    /// Fixed storage initialization is not a Plan binding or Show admission.
    pub fn plan(&self) -> Option<&conduit_face::PlannedMaskPlot> {
        None
    }
    pub(crate) fn healthy(&self) -> bool {
        self.input.as_ref().is_none_or(|input| input.is_live())
    }
}
impl StdHost {
    pub fn ready_selected_native_mask(&self) -> Option<&SelectedNativeMaskBase> {
        self.ready_selected_scanout()?
            .native_mask
            .as_ref()
            .filter(|mask| mask.healthy())
    }
    pub fn selected_native_mask_supports_input(&self) -> bool {
        self.ready_selected_native_mask()
            .is_some_and(|mask| mask.input.as_ref().is_some_and(|input| input.is_live()))
    }
}
