//! Exact pre-trigger validation and resource reservation for std kernel runs.
//!
//! Migrated profiles use this boundary instead of instantiating the legacy
//! runtime merely to validate a plan and hold its resource pools.

mod exact_profile;
use exact_profile::validate_exact_profile;

use crate::installed_std::state_storage_profile;
use conduit_core::{
    resource_binding_satisfies, HostAdvertisement, PlanFragment, PlanId, ResourceBinding,
    ResourceClassId, ResourceHealth, ResourceObservation, ResourcePoolId, SignId, PROTOCOL_VERSION,
};
use conduit_plan_lowering::lowering::lower_plan_fragment_for_profile;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoolUsage {
    pool_id: ResourcePoolId,
    class_id: ResourceClassId,
    capacity_units: u32,
    used_units: u32,
}

#[derive(Debug, Clone)]
pub(super) struct KernelResourceLedger {
    pools: Vec<PoolUsage>,
    instances: Vec<(conduit_core::CapabilityId, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KernelResourceReservation {
    plan_id: PlanId,
    bindings: Vec<ResourceBinding>,
    instances: Vec<(conduit_core::CapabilityId, u16)>,
}

impl KernelResourceLedger {
    pub(super) fn is_idle(&self) -> bool {
        self.pools.iter().all(|pool| pool.used_units == 0)
            && self.instances.iter().all(|(_, active)| *active == 0)
    }

    pub(super) fn new(advertisement: &HostAdvertisement) -> Result<Self, String> {
        if advertisement.protocol_version != PROTOCOL_VERSION {
            return Err("kernel host advertisement uses the wrong protocol version".to_string());
        }
        let mut pools = Vec::with_capacity(advertisement.resources.len());
        for offer in &advertisement.resources {
            if offer.pool_id.as_str().is_empty()
                || offer.class_id.as_str().is_empty()
                || offer.capacity_units == 0
                || pools
                    .iter()
                    .any(|pool: &PoolUsage| pool.pool_id == offer.pool_id)
            {
                return Err("kernel host resource offers are malformed".to_string());
            }
            pools.push(PoolUsage {
                pool_id: offer.pool_id.clone(),
                class_id: offer.class_id.clone(),
                capacity_units: offer.capacity_units,
                used_units: 0,
            });
        }
        let mut instances = Vec::with_capacity(advertisement.capabilities.len());
        for capability in &advertisement.capabilities {
            if instances
                .iter()
                .any(|(id, _)| id == &capability.capability_id)
            {
                return Err("kernel host capability identities are duplicated".into());
            }
            instances.push((capability.capability_id.clone(), 0));
        }
        Ok(Self { pools, instances })
    }

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
        for pool in &mut staged.pools {
            let requested = requested_units(bindings, &pool.pool_id, &pool.class_id)?;
            pool.used_units = pool
                .used_units
                .checked_add(requested)
                .filter(|total| *total <= pool.capacity_units)
                .ok_or_else(|| {
                    format!(
                        "pool member resource '{}' capacity is unavailable",
                        pool.pool_id.as_str()
                    )
                })?;
        }
        if bindings.iter().any(|binding| {
            !staged
                .pools
                .iter()
                .any(|pool| pool.pool_id == binding.pool_id && pool.class_id == binding.class_id)
        }) {
            return Err(
                "pool member resource binding is absent from the initialized ledger".into(),
            );
        }
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
            live.used_units = admitted.used_units;
        }
        for (live, admitted) in self.instances.iter_mut().zip(staged.instances) {
            live.1 = admitted.1;
        }
        Ok(reservations)
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
        validate_exact_profile(advertisement, fragment)?;

        let mut instances = Vec::new();
        for (id, used) in &mut self.instances {
            let requested = u16::try_from(
                fragment
                    .placements
                    .iter()
                    .filter(|placement| &placement.capability_id == id)
                    .count(),
            )
            .map_err(|_| "capability instance demand overflow".to_string())?;
            if requested == 0 {
                continue;
            }
            let offer = advertisement
                .capabilities
                .iter()
                .find(|offer| &offer.capability_id == id)
                .ok_or_else(|| "reserved capability is no longer offered".to_string())?;
            let total = used
                .checked_add(requested)
                .filter(|total| *total <= offer.limits.max_active_instances)
                .ok_or_else(|| {
                    format!(
                        "capability '{}' combined active-instance limit exceeded",
                        id.as_str()
                    )
                })?;
            *used = total;
            instances.push((id.clone(), requested));
        }
        if fragment.placements.iter().any(|placement| {
            !self
                .instances
                .iter()
                .any(|(id, _)| id == &placement.capability_id)
        }) {
            return Err("planned capability is absent from the initialized ledger".into());
        }

        if lowered.resources.len() != lowered.identity.resources.len() {
            return Err("lowered resource identity table width changed".to_string());
        }
        let mut bindings = Vec::with_capacity(lowered.resources.len());
        for (numeric, (node, resource, semantic)) in
            lowered.resources.iter().zip(&lowered.identity.resources)
        {
            if numeric.node != *node
                || numeric.binding.resource != *resource
                || numeric.binding.units != semantic.units
            {
                return Err("lowered numeric resource row lost its semantic identity".to_string());
            }
            bindings.push(semantic.clone());
        }

        for pool in &self.pools {
            let requested = requested_units(&bindings, &pool.pool_id, &pool.class_id)?;
            let total = pool.used_units.checked_add(requested).ok_or_else(|| {
                format!("resource pool '{}' usage overflowed", pool.pool_id.as_str())
            })?;
            if total > pool.capacity_units {
                return Err(format!(
                    "resource pool '{}' requires {} units above capacity {}",
                    pool.pool_id.as_str(),
                    total,
                    pool.capacity_units
                ));
            }
        }
        for binding in &bindings {
            if !self
                .pools
                .iter()
                .any(|pool| pool.pool_id == binding.pool_id && pool.class_id == binding.class_id)
            {
                return Err(format!(
                    "resource binding '{}' is not offered by this host",
                    binding.pool_id.as_str()
                ));
            }
        }
        for pool in &mut self.pools {
            let requested = requested_units(&bindings, &pool.pool_id, &pool.class_id)?;
            pool.used_units += requested;
        }
        Ok(KernelResourceReservation {
            plan_id: fragment.plan_id.clone(),
            bindings,
            instances,
        })
    }

    pub(super) fn release(&mut self, reservation: KernelResourceReservation) -> Result<(), String> {
        for (id, released) in &reservation.instances {
            let (_, used) = self
                .instances
                .iter_mut()
                .find(|(candidate, _)| candidate == id)
                .ok_or_else(|| "released capability is absent from the ledger".to_string())?;
            *used = used
                .checked_sub(*released)
                .ok_or_else(|| "capability release exceeded its reservation".to_string())?;
        }
        for pool in &mut self.pools {
            let released = requested_units(&reservation.bindings, &pool.pool_id, &pool.class_id)?;
            pool.used_units = pool.used_units.checked_sub(released).ok_or_else(|| {
                format!(
                    "plan '{}' release exceeded resource pool '{}' reservation",
                    reservation.plan_id.as_str(),
                    pool.pool_id.as_str(),
                )
            })?;
        }
        Ok(())
    }

    pub(super) fn observe_bindings(
        &self,
        advertisement: &HostAdvertisement,
        bindings: &[ResourceBinding],
        sign_ids: &[SignId],
    ) -> Result<Vec<ResourceObservation>, String> {
        if bindings.len() != sign_ids.len() {
            return Err(
                "resource observation Sign width does not match the sealed bindings".into(),
            );
        }
        let mut observations = Vec::with_capacity(bindings.len());
        for (binding, sign_id) in bindings.iter().zip(sign_ids) {
            if sign_id.as_str().is_empty() {
                return Err("resource observation Sign identity is empty".into());
            }
            let pool = self
                .pools
                .iter()
                .find(|pool| pool.pool_id == binding.pool_id && pool.class_id == binding.class_id)
                .ok_or_else(|| {
                    format!(
                        "sealed resource pool '{}' is absent from the current host ledger",
                        binding.pool_id.as_str()
                    )
                })?;
            observations.push(ResourceObservation {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                pool_id: pool.pool_id.clone(),
                class_id: pool.class_id.clone(),
                health: ResourceHealth::Ready,
                unreserved_units: pool.capacity_units - pool.used_units,
                utilized_units: pool.used_units,
                sign_id: sign_id.clone(),
            });
        }
        Ok(observations)
    }

    #[cfg(test)]
    fn allocation_capacity(&self) -> usize {
        self.pools.capacity()
    }
}

#[cfg(test)]
#[path = "kernel_partition_reservation_tests.rs"]
mod partition_tests;

fn requested_units(
    bindings: &[ResourceBinding],
    pool_id: &ResourcePoolId,
    class_id: &ResourceClassId,
) -> Result<u32, String> {
    bindings
        .iter()
        .filter(|binding| &binding.pool_id == pool_id && &binding.class_id == class_id)
        .try_fold(0_u32, |total, binding: &ResourceBinding| {
            total
                .checked_add(binding.units)
                .ok_or_else(|| format!("resource pool '{}' usage overflowed", pool_id.as_str()))
        })
}

#[cfg(test)]
mod tests;
