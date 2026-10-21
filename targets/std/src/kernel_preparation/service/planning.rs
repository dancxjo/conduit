//! Convenience entrance to the canonical planner's explicit region selection.
use super::*;
use conduit_core::*;
use conduit_planner::execution_regions::{seal_partitioned_execution, PartitionedRegionSelection};

impl InitializedRegionService<'_> {
    /// Select explicit placement groups against this initialized service. The
    /// input must have been planned against its current advertised generation.
    /// This creates a new sealed Plan; prepare still checks current availability
    /// and operation owners and atomically commits every reservation.
    pub fn select_plan_regions(
        &self,
        plan: Plan,
        groups: Vec<Vec<PlacementId>>,
        scratch_bytes: u32,
    ) -> Result<Plan, String> {
        let host = self.advertisement()?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err("region selection requires one local fragment".into());
        };
        if fragment.host_id != host.host_id
            || fragment.boot_id != host.boot_id
            || fragment.offer_generation != host.offer_generation
        {
            return Err("region selection Plan is stale for this initialized Host".into());
        }
        if groups.len() > usize::from(self.config.lanes)
            || self
                .config
                .stack_bytes
                .checked_add(scratch_bytes)
                .is_none_or(|bytes| bytes > self.config.worker_memory_bytes)
        {
            return Err("region selection exceeds the initialized lane/storage profile".into());
        }
        let contract = self.provider.resources[0]
            .compute
            .as_ref()
            .ok_or("initialized worker lane contract is missing")?;
        let selections = groups
            .into_iter()
            .enumerate()
            .map(|(index, placements)| PartitionedRegionSelection {
                region_id: format!(
                    "{}/region/{index}",
                    self.provider.provider_instance_id.as_str()
                )
                .into(),
                placements,
                execution_profile_id: self.config.execution_profile_id(),
                lane_resource: ResourceBinding {
                    pool_id: self.provider.resources[0].pool_id.clone(),
                    class_id: self.provider.resources[0].class_id.clone(),
                    units: 1,
                    compute: Some(ComputeReservation {
                        selected_lanes: 1,
                        service_guarantee: contract.service_guarantee,
                        architecture_base_id: contract.architecture_base_id.clone(),
                        architecture_base_kind: contract.architecture_base_kind,
                        topology_group_id: None,
                        performance_class: None,
                        nominal_clock_hz: None,
                    }),
                    protected: None,
                    content: None,
                },
                lane_base_id: self.provider.base_id.clone(),
                worker_storage: ExecutionWorkerStorage {
                    binding: ResourceBinding {
                        pool_id: self.provider.resources[1].pool_id.clone(),
                        class_id: self.provider.resources[1].class_id.clone(),
                        units: self.config.worker_memory_bytes,
                        compute: None,
                        protected: None,
                        content: None,
                    },
                    stack_bytes: self.config.stack_bytes,
                    scratch_bytes,
                },
            })
            .collect();
        seal_partitioned_execution(plan, selections)
            .map_err(|error| format!("region planning: {error:?}"))
    }
}
