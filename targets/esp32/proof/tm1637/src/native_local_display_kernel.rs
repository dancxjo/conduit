//! Same ordinary output kernel, locally prepared; no binding, Step or GPIO call.
use super::*;
use conduit_environment_runtime::display_output::{
    DisplaySignBudget, DisplayValueBudget, LocalDisplayInput, PreparedDisplayKernel,
};
use conduit_kernel as k;
use conduit_plan_lowering::local_tables::VerifiedLocalLowering;

impl NativeDisplay {
    pub fn prepare_local_kernel(
        &mut self,
        verified: &VerifiedLocalLowering<'_>,
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<PreparedDisplayKernel<k::HostedValueStore, k::HostedSignLog>, &'static str> {
        let lowered = verified.lowered();
        let frames = 2;
        let signs = DisplaySignBudget::derive(frames, lowered.sign_items, lowered.sign_bytes)
            .map_err(|_| "local-display-signs")?;
        let values = DisplayValueBudget::derive(frames, layout::MAXIMUM_VALUE_BYTES)
            .map_err(|_| "local-display-values")?;
        let needed = signs.storage_bytes().ok_or("local-display-signs")?
            + values.storage_bytes().map_err(|_| "local-display-values")?
            + core::mem::size_of::<crate::native_display::runtime::Raw>()
            + 8192;
        let memory = esp_alloc::HEAP.stats();
        if memory.size.saturating_sub(memory.current_usage) < needed {
            return Err("local-display-kernel-capacity");
        }
        let image = planning_source()?;
        let boundary = planning_boundary()?;
        let software = self
            .software
            .as_mut()
            .map_err(|_| "display-software-owner")?;
        let values = values.prepare().map_err(|_| "local-display-values")?;
        let signs = k::HostedSignLog::new_with_remote_storage(
            signs.ordinary_items(),
            signs.ordinary_bytes(),
            signs.remote_items(),
            signs.remote_bytes(),
        )
        .map_err(|_| "local-display-signs")?;
        PreparedDisplayKernel::prepare_local(
            LocalDisplayInput {
                verified,
                image: &image,
                boundary: &boundary,
                scope,
                maximum_frames: frames,
                maximum_value_bytes: layout::MAXIMUM_VALUE_BYTES,
            },
            software,
            values,
            signs,
        )
        .map_err(|_| "local-display-kernel")
    }
}
