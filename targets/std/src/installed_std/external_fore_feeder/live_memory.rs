//! Newly owned live feeder metadata. Shared queue slots retain their original
//! IMAGE lease; producer codec/control/thread backing remains independently owned.
use super::*;
impl LiveForeFeeder {
    fn image_backing_bytes(&self) -> Option<usize> {
        if self.initial_started
            || self.closed
            || self.full
            || self.queue.initial().is_some()
            || !self.queue.has_image_admission()
        {
            return None;
        }
        // Verify preparation state without charging shared storage a second time.
        self.queue.image_shared_backing_bytes()?;
        core::mem::size_of::<Self>()
            .checked_add(self.queue.image_clone_metadata_bytes()?)?
            .checked_add(
                self.targets
                    .capacity()
                    .checked_mul(core::mem::size_of::<(RemoteEndpointId, CordId)>())?,
            )?
            .checked_add(
                self.initial_targets
                    .capacity()
                    .checked_mul(core::mem::size_of::<(RemoteEndpointId, CordId)>())?,
            )
    }
}
impl PreparedForeInputs<'_> {
    pub(in crate::installed_std) fn image_live_backing_bytes(&self) -> Option<usize> {
        match self {
            Self::Live(feeder) => feeder.image_backing_bytes(),
            _ => None,
        }
    }
}
