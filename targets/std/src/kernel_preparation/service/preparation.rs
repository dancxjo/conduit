//! Exact per-Play preparation and joint admission; no worker is advanced here.
use super::*;
use conduit_composite::{
    KernelCompositeSignStorage, KernelOperationRegistry, PreparedRegionKernel,
};
use conduit_core::{Plan, ResourceObservation, SignId};
use conduit_plan_lowering::execution_regions::{ExecutionRegionLimits, PreparedExecutionRegions};
impl InitializedRegionService<'_> {
    pub fn prepared_region_count(&self) -> usize {
        self.prepared_count
    }
    pub fn prepared_plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }
    /// Observations retained as the admission basis, not a new availability
    /// reading after reservation. Use current_resource_observations for that.
    pub fn admission_observations(&self) -> &[ResourceObservation] {
        &self.admission_observations
    }
    pub fn failed_retirement(&self) -> Option<&RegionServiceRetirement> {
        self.retired.as_ref()
    }
    pub fn current_resource_observations(&mut self) -> Result<Vec<ResourceObservation>, String> {
        self.advertisement()?;
        let first = self.next_observation_sequence;
        let next = first
            .checked_add(self.provider.resources.len() as u64)
            .ok_or("worker observation sequence exhausted")?;
        let sign_ids: Vec<_> = (first..next)
            .map(|sequence| {
                SignId::from(format!(
                    "{}/resource-observation/{sequence}",
                    self.provider.provider_instance_id.as_str()
                ))
            })
            .collect();
        let observations = self.host.kernel_resources.observe_offers(
            &self.host.advertisement,
            &self.provider.resources,
            &sign_ids,
        )?;
        self.next_observation_sequence = next;
        Ok(observations)
    }
    pub fn prepare(
        &mut self,
        plan: Plan,
        registry: &KernelOperationRegistry,
    ) -> Result<(), String> {
        self.advertisement()?;
        if self.plan.is_some() {
            return Err("region service already owns a prepared Plan".into());
        }
        let [fragment] = plan.fragments.as_slice() else {
            return Err("Std region service requires one exact local fragment".into());
        };
        let prepared = PreparedExecutionRegions::prepare(
            &plan,
            &fragment.fragment_id,
            ExecutionRegionLimits {
                maximum_regions: usize::from(self.config.lanes),
                maximum_lanes: u32::from(self.config.lanes),
                maximum_nodes: usize::from(self.config.lanes) * 64,
                maximum_cords: usize::from(self.config.lanes) * 128,
            },
        )
        .map_err(|e| format!("worker Plan preparation: {e:?}"))?;
        for region in prepared.regions() {
            let region = region.admitted();
            let storage = region
                .scheduling
                .worker_storage()
                .ok_or("worker requires its partitioned storage profile")?;
            if region.execution_profile_id != self.config.execution_profile_id()
                || region.lane_base_id != self.provider.base_id
                || region.lane_resource.pool_id != self.provider.resources[0].pool_id
                || storage.binding.pool_id != self.provider.resources[1].pool_id
                || storage.stack_bytes != self.config.stack_bytes
                || storage.binding.units > self.config.worker_memory_bytes
            {
                return Err("region profile does not match the initialized worker service".into());
            }
        }
        let mut kernels = Vec::with_capacity(prepared.regions().len());
        for region in 0..prepared.regions().len() {
            let kernel = PreparedRegionKernel::prepare(
                &prepared,
                region,
                registry,
                KernelCompositeSignStorage::default(),
            )?;
            if kernel.maximum_transfer_bytes() > self.config.transfer_bytes as usize {
                return Err("region transfer exceeds the initialized mailbox profile".into());
            }
            kernels.push(kernel);
        }
        let observations = self.current_resource_observations()?;
        let reservation = self.host.kernel_resources.reserve_worker_resources(
            &self.host.advertisement,
            &prepared,
            &observations,
        )?;
        self.reservation = Some(reservation);
        self.admission_observations = observations;
        self.prepared_count = kernels.len();
        self.plan = Some(plan);
        for (lane, kernel) in kernels.into_iter().enumerate() {
            if let Err(error) = self.workers[lane].install(kernel) {
                let mut retirement = self.retire();
                retirement.record_host_error(format!("worker installation failed: {error:?}"));
                self.retired = Some(retirement);
                return Err(format!("worker installation failed: {error:?}"));
            }
        }
        Ok(())
    }
}
