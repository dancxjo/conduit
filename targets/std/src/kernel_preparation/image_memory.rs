//! Private cooperative execution/result backing custody; no RSS accounting.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// Constructed only by the trusted closed local accounting owner, never wire data.
#[derive(Debug)]
pub(crate) struct BodyMemoryReceipt {
    total_bytes: usize,
}
impl BodyMemoryReceipt {
    pub(crate) fn prepared_local(total_bytes: usize) -> Self {
        Self { total_bytes }
    }
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub(crate) fn prepared_native_mask(total_bytes: usize) -> Self {
        Self { total_bytes }
    }
    pub(crate) fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Shared only with existing prepared immutable Fore payload owners. A clone
/// retains the same charge; it does not reserve again or acknowledge delivery.
#[derive(Debug, Clone)]
pub(crate) struct ImageMemoryLease {
    inner: Arc<ImageMemoryCharge>,
}
#[derive(Debug)]
struct ImageMemoryCharge {
    used: Arc<AtomicUsize>,
    bytes: AtomicUsize,
}
impl ImageMemoryLease {
    pub(crate) fn retain_result(&mut self, bytes: usize) {
        let admitted = self.inner.bytes.load(Ordering::Acquire);
        assert!(
            bytes <= admitted,
            "result exceeds admitted execution highwater"
        );
        self.inner.bytes.store(bytes, Ordering::Release);
        self.inner
            .used
            .fetch_sub(admitted - bytes, Ordering::AcqRel);
    }
    pub(crate) fn owned_backing_bytes() -> usize {
        // Qualified std Arc layout: data plus strong/weak reference counts.
        core::mem::size_of::<ImageMemoryCharge>() + 2 * core::mem::size_of::<usize>()
    }
}
impl Drop for ImageMemoryCharge {
    fn drop(&mut self) {
        self.used
            .fetch_sub(self.bytes.load(Ordering::Acquire), Ordering::AcqRel);
    }
}
/// Affine initial construction custody. Identity and IMAGE are borrowed from
/// independently verified immutable inputs, so staging allocates no ID copies.
/// The one Arc counter allocation is explicitly charged, including its two
/// reference counts; allocator rounding and process RSS are not measured here.
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
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
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
        ledger.bind_staged_image_bounds(&image.manifest.bounds, self.used)
    }
}

fn reserve_image_memory(
    used: &Arc<AtomicUsize>,
    limit: usize,
    bytes: usize,
) -> Result<ImageMemoryLease, String> {
    if bytes == 0 {
        return Err("managed IMAGE receipt is empty".into());
    }
    used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        old.checked_add(bytes).filter(|next| *next <= limit)
    })
    .map_err(|_| "selected IMAGE managed heap backing capacity is unavailable")?;
    Ok(ImageMemoryLease {
        inner: Arc::new(ImageMemoryCharge {
            used: used.clone(),
            bytes: AtomicUsize::new(bytes),
        }),
    })
}

impl KernelResourceLedger {
    pub(crate) fn require_input_memory_owner(
        &self,
        lease: &ImageMemoryLease,
    ) -> Result<(), String> {
        self.require_available()?;
        if self
            .image_memory
            .as_ref()
            .is_some_and(|used| Arc::ptr_eq(used, &lease.inner.used))
        {
            Ok(())
        } else {
            Err("IMAGE input backing belongs to another memory owner".into())
        }
    }

    /// Reserve the independently computed fixed Base stores before allocation.
    /// This is not a complete Plan receipt, process RSS or preparation peak.
    pub(crate) fn reserve_managed_base_memory(
        &mut self,
        bytes: usize,
    ) -> Result<ImageMemoryLease, String> {
        self.require_available()?;
        let bounds = self
            .image_bounds
            .as_ref()
            .ok_or("managed Base requires checked IMAGE bounds")?;
        let limit = usize::try_from(bounds.heap_arena_bytes)
            .map_err(|_| "IMAGE heap exceeds local size")?;
        let used = self
            .image_memory
            .as_ref()
            .ok_or("IMAGE memory owner absent")?;
        reserve_image_memory(used, limit, bytes)
    }
    #[cfg(test)]
    pub(crate) fn image_memory_observer(&self) -> Arc<AtomicUsize> {
        self.image_memory.as_ref().unwrap().clone()
    }
    pub(crate) fn has_image_bounds(&self) -> bool {
        self.image_bounds.is_some()
    }
    pub(crate) fn reserve_body_memory(
        &mut self,
        advertisement: &HostAdvertisement,
        plans: &[(&Plan, bool)],
        receipt: &BodyMemoryReceipt,
    ) -> Result<(Vec<KernelResourceReservation>, ImageMemoryLease), String> {
        self.require_available()?;
        let bounds = self
            .image_bounds
            .as_ref()
            .ok_or("IMAGE memory requires selected construction")?;
        let limit = usize::try_from(bounds.heap_arena_bytes)
            .map_err(|_| "IMAGE heap bound exceeds local size")?;
        let used = self
            .image_memory
            .as_ref()
            .ok_or("IMAGE memory owner missing")?;
        let lease = reserve_image_memory(used, limit, receipt.total_bytes())?;
        self.image_receipt_reserving = true;
        let result = self.prepare_and_reserve_plans(advertisement, plans);
        self.image_receipt_reserving = false;
        Ok((result?, lease))
    }
}

#[cfg(test)]
#[path = "staged_image_memory_tests.rs"]
mod staged_tests;
