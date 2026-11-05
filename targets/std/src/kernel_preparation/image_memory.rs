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
        core::mem::size_of::<ImageMemoryCharge>()
    }
}
impl Drop for ImageMemoryCharge {
    fn drop(&mut self) {
        self.used
            .fetch_sub(self.bytes.load(Ordering::Acquire), Ordering::AcqRel);
    }
}
impl KernelResourceLedger {
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
            .ok_or("IMAGE memory owner absent")?
            .clone();
        if bytes == 0 {
            return Err("managed Base receipt is empty".into());
        }
        used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
            old.checked_add(bytes).filter(|next| *next <= limit)
        })
        .map_err(|_| "selected IMAGE managed Base backing capacity is unavailable")?;
        Ok(ImageMemoryLease {
            inner: Arc::new(ImageMemoryCharge {
                used,
                bytes: AtomicUsize::new(bytes),
            }),
        })
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
            .ok_or("IMAGE memory owner missing")?
            .clone();
        used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
            old.checked_add(receipt.total_bytes())
                .filter(|next| *next <= limit)
        })
        .map_err(|_| "selected IMAGE managed heap backing capacity is unavailable")?;
        let lease = ImageMemoryLease {
            inner: Arc::new(ImageMemoryCharge {
                used,
                bytes: AtomicUsize::new(receipt.total_bytes()),
            }),
        };
        self.image_receipt_reserving = true;
        let result = self.prepare_and_reserve_plans(advertisement, plans);
        self.image_receipt_reserving = false;
        Ok((result?, lease))
    }
}
