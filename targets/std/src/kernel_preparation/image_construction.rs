//! Verified initial Base custody transferred to the final ledger unchanged.
use super::image_memory::reserve_image_memory;
use super::*;
use std::sync::{atomic::AtomicUsize, Arc};
pub(crate) struct StagedImageMemory<'a> {
    config: &'a crate::StdHostConfig,
    image: &'a conduit_host_make::HostImage,
    used: Arc<AtomicUsize>,
    limit: usize,
}
impl<'a> StagedImageMemory<'a> {
    pub(crate) fn verified(
        config: &'a crate::StdHostConfig,
        image: &'a conduit_host_make::HostImage,
        checked: &conduit_host_make::CheckedHostConfiguration,
        catalog: &conduit_host_make::MakeCatalog,
    ) -> Result<Self, String> {
        super::image_configuration::verify_configuration_image(checked, image, catalog)?;
        if config.host_id.as_str().is_empty()
            || config.boot_id.as_str().is_empty()
            || config.offer_generation.0 == 0
        {
            return Err("staged IMAGE requires current intended Host/Boot identity".into());
        }
        let limit = usize::try_from(image.manifest.bounds.heap_arena_bytes)
            .map_err(|_| "IMAGE heap exceeds local size")?;
        let metadata = Self::counter_backing_bytes();
        if metadata > limit {
            return Err("IMAGE heap cannot retain its memory counter".into());
        }
        Ok(Self {
            config,
            image,
            used: Arc::new(AtomicUsize::new(metadata)),
            limit,
        })
    }
    pub(crate) fn counter_backing_bytes() -> usize {
        core::mem::size_of::<AtomicUsize>() + 2 * core::mem::size_of::<usize>()
    }
    pub(crate) fn reserve(&self, bytes: usize) -> Result<ImageMemoryLease, String> {
        reserve_image_memory(&self.used, self.limit, bytes)
    }
    pub(crate) fn owns(&self, lease: &ImageMemoryLease) -> bool {
        lease.belongs_to(&self.used)
    }
    /// Initial ledger construction only. Existing leases keep exactly the same
    /// counter; binding neither resets their charge nor reserves them again.
    pub(crate) fn bind(
        self,
        ledger: &mut KernelResourceLedger,
        actual: &HostAdvertisement,
        image: &conduit_host_make::HostImage,
    ) -> Result<(), String> {
        if image != self.image
            || actual.host_id != self.config.host_id
            || actual.boot_id != self.config.boot_id
            || actual.offer_generation != self.config.offer_generation
            || ledger.identity.0 != actual.host_id
            || ledger.identity.1 != actual.boot_id
            || ledger.identity.2 != actual.offer_generation
        {
            return Err("staged IMAGE memory differs from final Host/Boot/IMAGE".into());
        }
        if ledger.image_bounds.is_some() || !ledger.is_idle() || ledger.image_memory.is_some() {
            return Err("staged IMAGE requires a fresh idle final ledger".into());
        }
        ledger.image_bounds = Some(image.manifest.bounds.clone());
        ledger.image_memory = Some(self.used);
        Ok(())
    }
}
