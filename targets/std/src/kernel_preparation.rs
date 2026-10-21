//! Exact pre-trigger validation and resource reservation for std kernel runs.
//!
//! Migrated profiles use this boundary instead of instantiating the legacy
//! runtime merely to validate a plan and hold its resource pools.

mod exact_profile;
use exact_profile::validate_exact_profile;

use crate::installed_std::state_storage_profile;
use conduit_core::{
    resource_binding_satisfies, HostAdvertisement, Plan, PlanFragment, PlanId, ResourceBinding,
    ResourceHealth, ResourceObservation, SignId, PROTOCOL_VERSION,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_for_profile, lower_plan_fragment_for_profile_from_plan,
};

mod pool_usage;
use pool_usage::PoolUsage;
mod region_resources;
mod service;
pub use service::{
    InitializedRegionService, RegionAdvance, RegionExecution, RegionRoundState,
    RegionServiceConfig, RegionServiceRetirement, RegionWorkerCommand, RegionWorkerError,
    RegionWorkerEvidence, RegionWorkerReply,
};
mod worker;

mod ledger;
pub(crate) use ledger::{KernelResourceLedger, KernelResourceReservation};
mod placement_resources;

impl KernelResourceLedger {
    pub(super) fn prepare_and_reserve(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
    ) -> Result<KernelResourceReservation, String> {
        self.prepare_and_reserve_with_continuity(advertisement, fragment, false)
    }

    /// Reserve one dynamic shared-pool member through the same capability and
    /// resource ledger as static placements. The plan has already sealed the
    /// exact binding vector; this boundary revalidates it against the current
    /// capability requirements and commits atomically.
    pub(super) fn reserve_pool_member(
        &mut self,
        advertisement: &HostAdvertisement,
        plan_id: PlanId,
        capability_id: &conduit_core::CapabilityId,
        bindings: &[ResourceBinding],
    ) -> Result<KernelResourceReservation, String> {
        let capability = advertisement
            .capabilities
            .iter()
            .find(|offer| &offer.capability_id == capability_id)
            .ok_or_else(|| "pool member capability is no longer offered".to_string())?;
        if bindings.len() != capability.resource_requirements.len() {
            return Err("pool member resource binding width changed".into());
        }
        for requirement in &capability.resource_requirements {
            let matches = bindings
                .iter()
                .filter(|binding| binding.class_id == requirement.class_id)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err("pool member resource requirement is not bound exactly once".into());
            }
            let binding = matches[0];
            let offer = advertisement
                .resources
                .iter()
                .find(|offer| {
                    offer.pool_id == binding.pool_id && offer.class_id == binding.class_id
                })
                .ok_or_else(|| "pool member resource binding is no longer offered".to_string())?;
            if !resource_binding_satisfies(binding, requirement, offer) {
                return Err(
                    "pool member resource binding no longer satisfies its capability".into(),
                );
            }
        }

        let mut staged = self.clone();
        let (_, used) = staged
            .instances
            .iter_mut()
            .find(|(id, _)| id == capability_id)
            .ok_or_else(|| {
                "pool member capability is absent from the initialized ledger".to_string()
            })?;
        *used = used
            .checked_add(1)
            .filter(|total| *total <= capability.limits.max_active_instances)
            .ok_or_else(|| "pool member active-instance capacity is unavailable".to_string())?;
        PoolUsage::reserve_all(&mut staged.pools, bindings)?;
        *self = staged;
        Ok(KernelResourceReservation {
            plan_id,
            bindings: bindings.to_vec(),
            instances: vec![(capability_id.clone(), 1)],
        })
    }

    pub(super) fn prepare_and_reserve_with_continuity(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        continuity: bool,
    ) -> Result<KernelResourceReservation, String> {
        self.prepare_and_reserve_partitions(advertisement, &[(fragment, continuity)])?
            .pop()
            .ok_or_else(|| "single partition reservation disappeared".to_string())
    }

    /// Admit the complete local workload before committing any pool usage.
    /// Each reservation retains its original Plan identity. BodyPlan/workset
    /// validation remains the caller's responsibility; this grants no Play.
    pub(super) fn prepare_and_reserve_partitions(
        &mut self,
        advertisement: &HostAdvertisement,
        partitions: &[(&PlanFragment, bool)],
    ) -> Result<Vec<KernelResourceReservation>, String> {
        if partitions.is_empty() || partitions.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("local workload partition count exceeds the admitted profile".into());
        }
        for (index, (fragment, _)) in partitions.iter().enumerate() {
            if partitions[..index].iter().any(|(prior, _)| {
                prior.plan_id == fragment.plan_id && prior.fragment_id == fragment.fragment_id
            }) {
                return Err("duplicate local workload partition".into());
            }
        }
        // Staging is pre-play, finite, and includes all existing reservations.
        // A late invalid partition or combined shortage discards the candidate
        // ledger without requiring fallible rollback of the live ledger.
        let mut staged = self.clone();
        let mut reservations = Vec::with_capacity(partitions.len());
        for (fragment, continuity) in partitions {
            reservations.push(staged.reserve_partition(advertisement, fragment, *continuity)?);
        }
        for (live, admitted) in self.pools.iter_mut().zip(staged.pools) {
            live.copy_usage_from(&admitted);
        }
        for (live, admitted) in self.instances.iter_mut().zip(staged.instances) {
            live.1 = admitted.1;
        }
        Ok(reservations)
    }

    /// Reserve Body partitions through their complete sealed Plans. An
    /// activation-bearing PlanId cannot be verified from a fragment alone.
    pub(super) fn prepare_and_reserve_plans(
        &mut self,
        advertisement: &HostAdvertisement,
        plans: &[(&Plan, bool)],
    ) -> Result<Vec<KernelResourceReservation>, String> {
        if plans.is_empty() || plans.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("local workload partition count exceeds the admitted profile".into());
        }
        let mut staged = self.clone();
        let mut reservations = Vec::with_capacity(plans.len());
        for (index, (plan, continuity)) in plans.iter().enumerate() {
            let fragment = plan
                .fragments
                .first()
                .filter(|_| plan.fragments.len() == 1)
                .ok_or_else(|| "local Body Plan requires exactly one fragment".to_string())?;
            if plans[..index].iter().any(|(prior, _)| {
                prior.fragments.iter().any(|part| {
                    part.plan_id == fragment.plan_id && part.fragment_id == fragment.fragment_id
                })
            }) {
                return Err("duplicate local workload partition".into());
            }
            reservations.push(staged.reserve_plan(advertisement, plan, fragment, *continuity)?);
        }
        for (live, admitted) in self.pools.iter_mut().zip(staged.pools) {
            live.copy_usage_from(&admitted);
        }
        for (live, admitted) in self.instances.iter_mut().zip(staged.instances) {
            live.1 = admitted.1;
        }
        Ok(reservations)
    }

    fn reserve_plan(
        &mut self,
        advertisement: &HostAdvertisement,
        plan: &Plan,
        fragment: &PlanFragment,
        continuity: bool,
    ) -> Result<KernelResourceReservation, String> {
        let mut profile = state_storage_profile();
        if continuity {
            profile = profile.with_owned_state_continuity();
        }
        let lowered =
            lower_plan_fragment_for_profile_from_plan(plan, &fragment.fragment_id, profile)
                .map_err(|error| format!("kernel preparation lowering: {error:?}"))?;
        self.reserve_verified_partition_with_lowered(advertisement, fragment, lowered)
    }

    fn reserve_partition(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        continuity: bool,
    ) -> Result<KernelResourceReservation, String> {
        let mut profile = state_storage_profile();
        if continuity {
            profile = profile.with_owned_state_continuity();
        }
        let lowered = lower_plan_fragment_for_profile(fragment, profile)
            .map_err(|error| format!("kernel preparation lowering: {error:?}"))?;
        self.reserve_verified_partition_with_lowered(advertisement, fragment, lowered)
    }

    fn reserve_verified_partition_with_lowered(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
    ) -> Result<KernelResourceReservation, String> {
        validate_exact_profile(advertisement, fragment)?;

        self.reserve_verified_placement_resources(advertisement, fragment, &lowered)
    }

    #[cfg(test)]
    fn allocation_capacity(&self) -> usize {
        self.pools.capacity()
    }
}

#[cfg(test)]
#[path = "kernel_partition_reservation_tests.rs"]
mod partition_tests;

#[cfg(test)]
mod tests;
