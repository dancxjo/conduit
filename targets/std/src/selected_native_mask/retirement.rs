//! Explicit exact-Plan retirement before changed directional Lines are admitted.
use super::*;

impl StdHost {
    /// Retire the exact completed Plan before preparing a replacement encounter.
    /// Frame/interaction evidence must already be disposed through its original
    /// ledger. Fixed stores, the physical provider and displayed pixels survive.
    /// An uncertain retirement remains terminal, never reusable capacity.
    pub fn retire_selected_native_mask_plan(&mut self) -> Result<(), String> {
        self.kernel_resources.require_available()?;
        let mut mask = self
            .selected_scanout
            .as_mut()
            .and_then(|scanout| scanout.native_mask.take())
            .ok_or("selected native Mask absent during Plan retirement")?;
        let result = mask.validate_binding(self).and_then(|()| {
            if matches!(&mask.stores, Stores::Unbound { .. }) {
                Ok(())
            } else {
                mask.retire_bound_plan(self)
            }
        });
        // Even failure must retain the original reservation/quarantine owners.
        if let Some(scanout) = self.selected_scanout.as_mut() {
            scanout.native_mask = Some(mask);
        }
        result
    }
}

impl SelectedNativeMaskBase {
    pub(super) fn retire_bound_plan(&mut self, host: &mut StdHost) -> Result<(), String> {
        host.kernel_resources.require_available()?;
        let Stores::Bound { renderer, .. } = &self.stores else {
            return Err("selected native Mask has no bound Plan to retire".into());
        };
        if !renderer.can_retire_plan() {
            return Err("selected Mask Plan still owns an active child Play or frame".into());
        }
        let physical = host
            .ready_selected_scanout()
            .ok_or("selected output is lost")?;
        let (width, height) = (
            physical.initialized().width(),
            physical.initialized().height(),
        );
        let previous = core::mem::replace(
            &mut self.stores,
            Stores::Lost {
                _plan_custody: Vec::new(),
                _metadata_pool: None,
            },
        );
        let Stores::Bound {
            renderer,
            mut _plan_custody,
            _metadata_pool,
            _metadata_heap,
            _kernel_metadata_heap,
        } = previous
        else {
            return Err("selected native Plan changed during retirement".into());
        };
        // Returned retained storage proves the prior child drained and handed
        // off its complete evidence. Destroy old actual stores first: the fixed
        // IMAGE/pool charge will cover the replacement, never two allocations.
        drop(renderer);
        drop(_metadata_heap);
        drop(_kernel_metadata_heap);
        while let Some(reservation) = _plan_custody.pop() {
            if let Err(error) = host.kernel_resources.release(reservation) {
                // A partial ledger retirement is uncertain, not reusable free
                // capacity. Preserve remaining tokens and block later Plays.
                host.kernel_resources.quarantine_retirement(_plan_custody);
                self.stores = Stores::Lost {
                    _plan_custody: Vec::new(),
                    _metadata_pool: Some(_metadata_pool),
                };
                return Err(error);
            }
        }
        if let Err((token, error)) = host.kernel_resources.release_base_backing(_metadata_pool) {
            host.kernel_resources.quarantine_retirement(Vec::new());
            self.stores = Stores::Lost {
                _plan_custody: Vec::new(),
                _metadata_pool: Some(token),
            };
            return Err(error);
        }
        if !self.backing.matches_ledger(&host.kernel_resources) {
            return Err("selected fixed store reservation was lost during Plan retirement".into());
        }
        let kernel = UnboundNativeMaskStorage::prepare(
            NativeMaskStorageProfile::Standard,
            self.receipt.mask.managed_store_bytes,
        )
        .map_err(|error| format!("selected replacement kernel preparation: {error:?}"))?;
        let raster = PreparedCompositorBacking::prepare(
            SURFACE,
            u16::try_from(width).map_err(|_| "selected replacement width")?,
            u16::try_from(height).map_err(|_| "selected replacement height")?,
            self.receipt.compositor_bytes,
        )
        .map_err(|error| format!("selected replacement raster preparation: {error:?}"))?;
        if kernel.backing() != self.receipt.mask
            || raster.backing_bytes() != self.receipt.compositor_bytes
        {
            return Err("selected replacement stores differ from admitted fixed backing".into());
        }
        self.stores = Stores::Unbound { kernel, raster };
        Ok(())
    }
}
