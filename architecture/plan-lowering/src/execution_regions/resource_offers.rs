//! Current-offer checks for exact region lane and worker-memory demand. No reservation occurs here.
use alloc::vec::Vec;
use conduit_core::{
    resource_binding_satisfies, ComputeRequirement, ComputeTopologyRequirement, HostAdvertisement,
    ResourceBinding, ResourceHealth, ResourceObservation, ResourceRequirement, PROTOCOL_VERSION,
};

use super::PreparedExecutionRegions;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegionResourceOfferError {
    StaleHost,
    AmbiguousPool,
    MissingPool,
    BindingMismatch,
    MissingObservation,
    StaleObservation,
    InvalidObservation,
    CapacityExceeded,
}

/// A checked view of the exact bindings and current observation basis. It is
/// deliberately not a reservation token; the Host must commit all demand to
/// its concrete ledger atomically before installing or starting execution.
#[derive(Debug)]
pub struct CheckedRegionResources<'prepared, 'host> {
    plan_id: &'prepared conduit_core::PlanId,
    bindings: Vec<&'prepared ResourceBinding>,
    host: &'host HostAdvertisement,
    observations: &'host [ResourceObservation],
}

impl<'prepared, 'host> CheckedRegionResources<'prepared, 'host> {
    pub fn plan_id(&self) -> &'prepared conduit_core::PlanId {
        self.plan_id
    }
    pub fn bindings(&self) -> &[&'prepared ResourceBinding] {
        &self.bindings
    }
    pub fn host(&self) -> &'host HostAdvertisement {
        self.host
    }
    pub fn observations(&self) -> &'host [ResourceObservation] {
        self.observations
    }
}

impl PreparedExecutionRegions<'_> {
    pub fn check_resource_offers<'prepared, 'host>(
        &'prepared self,
        host: &'host HostAdvertisement,
        observations: &'host [ResourceObservation],
    ) -> Result<CheckedRegionResources<'prepared, 'host>, RegionResourceOfferError> {
        let fragment = self.fragment();
        if host.protocol_version != PROTOCOL_VERSION
            || fragment.host_id != host.host_id
            || fragment.boot_id != host.boot_id
            || fragment.offer_generation != host.offer_generation
        {
            return Err(RegionResourceOfferError::StaleHost);
        }
        let bindings: Vec<_> = self
            .regions()
            .iter()
            .flat_map(|r| {
                core::iter::once(&r.admitted().lane_resource).chain(
                    r.admitted()
                        .scheduling
                        .worker_storage()
                        .map(|storage| &storage.binding),
                )
            })
            .collect();
        for binding in &bindings {
            let mut offers = host
                .resources
                .iter()
                .filter(|o| o.pool_id == binding.pool_id);
            let offer = offers.next().ok_or(RegionResourceOfferError::MissingPool)?;
            if offers.next().is_some() {
                return Err(RegionResourceOfferError::AmbiguousPool);
            }
            let compute_requirement = if let Some(compute) = &binding.compute {
                Some(ComputeRequirement {
                    minimum_lanes: binding.units,
                    preferred_lanes: binding.units,
                    maximum_lanes: binding.units,
                    minimum_service_guarantee: compute.service_guarantee,
                    topology: compute.topology_group_id.as_ref().map(|_| {
                        ComputeTopologyRequirement {
                            same_numa_domain: false,
                            same_cache_domain: false,
                            performance_class: compute.performance_class.clone(),
                        }
                    }),
                })
            } else {
                if binding.class_id.as_str() != conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS {
                    return Err(RegionResourceOfferError::BindingMismatch);
                }
                None
            };
            let requirement = ResourceRequirement {
                class_id: binding.class_id.clone(),
                units: binding.units,
                protected_role: None,
                content: None,
                compute: compute_requirement,
            };
            if !resource_binding_satisfies(binding, &requirement, offer) {
                return Err(RegionResourceOfferError::BindingMismatch);
            }
            let mut current = observations.iter().filter(|o| o.pool_id == binding.pool_id);
            let observation = current
                .next()
                .ok_or(RegionResourceOfferError::MissingObservation)?;
            if current.next().is_some() {
                return Err(RegionResourceOfferError::InvalidObservation);
            }
            if observation.host_id != host.host_id
                || observation.boot_id != host.boot_id
                || observation.offer_generation != host.offer_generation
            {
                return Err(RegionResourceOfferError::StaleObservation);
            }
            if observation.class_id != binding.class_id
                || observation.sign_id.as_str().is_empty()
                || observation.health != ResourceHealth::Ready
                || observation.unreserved_units > offer.capacity_units
                || observation.utilized_units > offer.capacity_units
            {
                return Err(RegionResourceOfferError::InvalidObservation);
            }
            // Charge region reservations AND placement demand against the same
            // pool. Checking each region separately would over-admit shared pools.
            let demand = bindings
                .iter()
                .copied()
                .chain(fragment.placements.iter().flat_map(|p| &p.resources))
                .filter(|b| b.pool_id == binding.pool_id)
                .try_fold(0u32, |total, b| total.checked_add(b.units))
                .ok_or(RegionResourceOfferError::CapacityExceeded)?;
            if demand > offer.capacity_units || demand > observation.unreserved_units {
                return Err(RegionResourceOfferError::CapacityExceeded);
            }
            if let Some(group_id) = binding
                .compute
                .as_ref()
                .and_then(|c| c.topology_group_id.as_ref())
            {
                let contract = offer
                    .compute
                    .as_ref()
                    .ok_or(RegionResourceOfferError::BindingMismatch)?;
                let mut groups = contract
                    .topology_groups
                    .iter()
                    .filter(|g| &g.group_id == group_id);
                let group = groups
                    .next()
                    .ok_or(RegionResourceOfferError::BindingMismatch)?;
                if groups.next().is_some() {
                    return Err(RegionResourceOfferError::BindingMismatch);
                }
                let group_demand = bindings
                    .iter()
                    .copied()
                    .chain(fragment.placements.iter().flat_map(|p| &p.resources))
                    .filter(|b| {
                        b.pool_id == binding.pool_id
                            && b.compute
                                .as_ref()
                                .is_some_and(|c| c.topology_group_id.as_ref() == Some(group_id))
                    })
                    .try_fold(0u32, |total, b| total.checked_add(b.units))
                    .ok_or(RegionResourceOfferError::CapacityExceeded)?;
                if group_demand > group.lane_capacity {
                    return Err(RegionResourceOfferError::CapacityExceeded);
                }
            }
        }
        Ok(CheckedRegionResources {
            plan_id: &self.plan().plan_id,
            bindings,
            host,
            observations,
        })
    }
}
