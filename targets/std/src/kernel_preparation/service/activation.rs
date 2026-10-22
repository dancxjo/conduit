//! Exact Play activation followed by bounded, nonblocking lane dispatch.
use super::*;
use conduit_kernel::{CanonicalValue, CordId};

/// Cord identities are the original Plan identities, never dense lane ordinals.
pub enum RegionWorkerCommand<'a> {
    Step,
    Offer(CordId),
    Admit {
        cord: CordId,
        sequence: u64,
        bytes: &'a [u8],
    },
    Delivered {
        cord: CordId,
        sequence: u64,
    },
    Terminal(CordId),
    Abnormal(CordId),
    Close(CordId),
    CloseAbnormal(CordId, CanonicalValue),
    Cancel,
}
impl InitializedRegionService<'_> {
    pub fn active_play(&self) -> Option<&conduit_core::ActivePlayIdentity> {
        self.active_play.as_ref()
    }
    /// Starts permission on every installed lane before exposing activation.
    /// No Back runs until the caller explicitly dispatches a command.
    pub fn start(&mut self) -> Result<&conduit_core::ActivePlayIdentity, String> {
        self.advertisement()?;
        if self.active_play.is_some() {
            return Err("region service already started".into());
        }
        if self.reservation.is_none() || self.prepared_count == 0 {
            return Err("region service has no admitted prepared Plan".into());
        }
        let plan = self.plan.as_ref().ok_or("region service has no Plan")?;
        let issued = self.host.issue_kernel_play(&plan.fragments[0])?;
        // Retain the consumed identity even if one start acknowledgement fails.
        self.active_play = Some(issued.identity().clone());
        for lane in 0..self.prepared_count {
            if let Err(error) = self.workers[lane].start() {
                let mut retirement = self.retire();
                retirement.record_host_error(format!("worker activation failed: {error:?}"));
                self.retired = Some(retirement);
                return Err(format!("worker activation failed: {error:?}"));
            }
        }
        Ok(self.active_play.as_ref().expect("activation retained"))
    }
    fn active_worker(
        &mut self,
        lane: usize,
    ) -> Result<&mut worker::InitializedRegionWorker, RegionWorkerError> {
        if self.active_play.is_none() {
            return Err(RegionWorkerError::NotStarted);
        }
        if lane >= self.prepared_count {
            return Err(RegionWorkerError::InvalidLane);
        }
        self.workers.get_mut(lane).ok_or(RegionWorkerError::Stopped)
    }
    /// One attempt, with no retry, allocation or implicit stepping of other lanes.
    pub fn dispatch(
        &mut self,
        lane: usize,
        command: RegionWorkerCommand<'_>,
    ) -> Result<(), RegionWorkerError> {
        let worker = self.active_worker(lane)?;
        match command {
            RegionWorkerCommand::Step => worker.step(),
            RegionWorkerCommand::Offer(cord) => worker.offer(cord),
            RegionWorkerCommand::Admit {
                cord,
                sequence,
                bytes,
            } => worker.admit(cord, sequence, bytes),
            RegionWorkerCommand::Delivered { cord, sequence } => worker.delivered(cord, sequence),
            RegionWorkerCommand::Terminal(cord) => worker.terminal(cord),
            RegionWorkerCommand::Abnormal(cord) => worker.abnormal(cord),
            RegionWorkerCommand::Close(cord) => worker.close(cord),
            RegionWorkerCommand::CloseAbnormal(cord, value) => worker.close_abnormal(cord, value),
            RegionWorkerCommand::Cancel => worker.cancel(),
        }
    }
    /// Offer payload is copied into caller-owned storage before releasing the
    /// mailbox. A short output buffer leaves the reply available for retry.
    pub fn poll(
        &mut self,
        lane: usize,
        output: &mut [u8],
    ) -> Result<Option<Result<RegionWorkerReply, RegionWorkerError>>, RegionWorkerError> {
        self.active_worker(lane)?.take_reply(output)
    }
}
