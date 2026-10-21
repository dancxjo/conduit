//! Exclusive controller ownership prevents manual mailbox commands interleaving.
use super::controller::{RegionController, Route};
use super::*;
use conduit_plan_lowering::execution_regions::{ExecutionRegionLimits, PreparedExecutionRegions};

pub struct RegionExecution<'host> {
    service: InitializedRegionService<'host>,
    controller: RegionController,
}
impl<'host> InitializedRegionService<'host> {
    /// Allocates finite controller backing before activation. A service already
    /// activated through the manual API cannot be adopted with unknown replies.
    pub fn into_execution(mut self) -> Result<RegionExecution<'host>, String> {
        self.advertisement()?;
        if self.active_play.is_some() {
            return Err("manual region Play is already active".into());
        }
        let plan = self.plan.as_ref().ok_or("no prepared region Plan")?;
        let prepared = PreparedExecutionRegions::prepare(
            plan,
            &plan.fragments[0].fragment_id,
            ExecutionRegionLimits {
                maximum_regions: usize::from(self.config.lanes),
                maximum_lanes: u32::from(self.config.lanes),
                maximum_nodes: usize::from(self.config.lanes) * 64,
                maximum_cords: usize::from(self.config.lanes) * 128,
            },
        )
        .map_err(|e| format!("controller preparation: {e:?}"))?;
        let routes = prepared
            .transfers()
            .iter()
            .map(|t| Route::new(t.cord(), t.source_region(), t.sink_region()))
            .collect();
        let controller = RegionController::new(routes, self.config.transfer_bytes as usize);
        self.start()?;
        Ok(RegionExecution {
            service: self,
            controller,
        })
    }
}
impl RegionExecution<'_> {
    pub fn active_play(&self) -> &conduit_core::ActivePlayIdentity {
        self.service
            .active_play()
            .expect("execution owns an active Play")
    }
    /// Managed controller backing, additional to initialized Base mailboxes and
    /// per-worker receipts; excludes allocator bookkeeping and thread overhead.
    pub fn controller_backing_bytes(&self) -> usize {
        self.controller.backing_bytes()
    }
    pub fn advance(&mut self) -> Result<RegionAdvance, RegionWorkerError> {
        self.controller
            .advance(&mut self.service.workers[..self.service.prepared_count])
    }
    pub fn shutdown(self) -> RegionServiceRetirement {
        let failure = self.controller.failure();
        let mut retirement = self.service.shutdown();
        retirement.controller_error = failure;
        retirement
    }
}
