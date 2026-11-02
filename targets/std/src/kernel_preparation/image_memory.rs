//! Private cooperative execution/result backing custody; no RSS accounting.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// Constructed only by the trusted closed tick accounting owner, never wire data.
#[derive(Debug)]
pub(crate) struct BodyMemoryReceipt {
    total_bytes: usize,
}
impl BodyMemoryReceipt {
    pub(crate) fn prepared_tick(total_bytes: usize) -> Self {
        Self { total_bytes }
    }
    pub(crate) fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

#[derive(Debug)]
pub(crate) struct ImageMemoryLease {
    used: Arc<AtomicUsize>,
    bytes: usize,
}
impl ImageMemoryLease {
    pub(crate) fn retain_result(&mut self, bytes: usize) {
        assert!(
            bytes <= self.bytes,
            "result exceeds admitted execution highwater"
        );
        self.used.fetch_sub(self.bytes - bytes, Ordering::AcqRel);
        self.bytes = bytes;
    }
}
impl Drop for ImageMemoryLease {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
impl KernelResourceLedger {
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
            used,
            bytes: receipt.total_bytes(),
        };
        self.image_receipt_reserving = true;
        let result = self.prepare_and_reserve_plans(advertisement, plans);
        self.image_receipt_reserving = false;
        Ok((result?, lease))
    }
}
