//! Retained read-only Mask stores and explicit previous-Play evidence custody.
use super::*;

impl NativeFaceMask {
    /// Initialize finite read-only Mask stores after the caller reserves their
    /// backing. This receipt excludes whole-Host/preparation peak and scanout.
    /// A failed child retires these stores; later Shows never recreate them.
    pub fn initialize_retained_backing(
        &mut self,
        maximum_bytes: usize,
    ) -> Result<NativeMaskBackingReceipt, NativeFaceMaskError> {
        if self.retained_backing || self.session.is_some() || self.publication.is_some() {
            return Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure));
        }
        let storage = RetainedNativeMaskStorage::prepare(
            &self.mask.planned_mask,
            self.producer.storage_profile(),
            maximum_bytes,
        )
        .map_err(NativeFaceMaskError::Mask)?;
        let receipt = storage.backing();
        self.retained_storage = Some(storage);
        self.retained_backing = true;
        self.local_input_available = false;
        Ok(receipt)
    }

    /// Explicit one-slot handoff of complete previous child evidence. Until this
    /// succeeds another retained Show is pressured. Taking is not durable ACK.
    pub fn take_retired_evidence(
        &mut self,
    ) -> Result<Option<RetiredNativeMaskEvidence>, NativeFaceMaskError> {
        if !self.retained_backing {
            return Ok(None);
        }
        let Some(session) = self.session.take() else {
            return Ok(None);
        };
        let (storage, evidence) = session
            .retire_retained()
            .map_err(NativeFaceMaskError::Mask)?;
        self.retained_storage = Some(storage);
        Ok(Some(evidence))
    }
}

#[cfg(test)]
#[path = "backing/tests.rs"]
mod tests;
