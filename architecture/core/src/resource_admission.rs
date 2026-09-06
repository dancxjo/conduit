//! Atomic ownership of finite Host resources and transient compute lanes.
//!
//! Planning can predict that a realization fits. This module represents the
//! later resource-owner decision: all exact bindings are reserved against one
//! current Host/Boot/offer generation or none are. Physical lane assignment is
//! retained only as active-Play truth and never enters Plan identity.

use alloc::{collections::BTreeSet, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    resource_binding_satisfies, ActivePlayId, BaseExecutionLaneId, BootId, ComputeLaneAssignment,
    HostAdvertisement, HostId, OfferGeneration, PlacementId, PlanId, ResourceBinding,
    ResourceHealth, ResourceObservation, ResourceRequirement, SignId,
};

pub const MAXIMUM_RESOURCE_ADMISSIONS: usize = 64;
pub const MAXIMUM_BINDINGS_PER_ADMISSION: usize = 64;
pub const MAXIMUM_TRANSIENT_LANE_ASSIGNMENTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAdmissionItem {
    pub requirement: ResourceRequirement,
    pub binding: ResourceBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAdmissionRequest {
    pub plan_id: PlanId,
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub items: Vec<ResourceAdmissionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAdmission {
    pub plan_id: PlanId,
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub items: Vec<ResourceAdmissionItem>,
    pub observation_sign_ids: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeAssignment {
    pub plan_id: PlanId,
    pub placement_id: PlacementId,
    pub active_play_id: ActivePlayId,
    pub lanes: Vec<ComputeLaneAssignment>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceReleaseCause {
    Completed,
    Cancelled,
    FailedStart,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasedResourceAdmission {
    pub admission: ResourceAdmission,
    pub cause: ResourceReleaseCause,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceAdmissionRefusal {
    Empty,
    CapacityExceeded,
    DuplicatePlanPlacement,
    ForeignHost,
    StaleOffer,
    MissingObservation,
    StaleObservation,
    Unavailable,
    InvalidBinding,
    CompetingResourceWriter,
    Overcommitted,
    UnknownAdmission,
    AssignmentCapacityExceeded,
    NoComputeEntitlement,
    ForeignPlayOrPlacement,
    DuplicateLane,
    TooManyLanes,
}

impl core::fmt::Display for ResourceAdmissionRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "resource admission refused: {self:?}")
    }
}

/// One boot-scoped resource owner. Mutations occur only after complete
/// validation, so a refusal leaves reservations and assignments unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAdmissionOwner {
    host: HostAdvertisement,
    admissions: Vec<ResourceAdmission>,
    assignments: Vec<ComputeAssignment>,
}

impl ResourceAdmissionOwner {
    pub fn new(host: HostAdvertisement) -> Self {
        Self {
            host,
            admissions: Vec::new(),
            assignments: Vec::new(),
        }
    }

    pub fn admissions(&self) -> &[ResourceAdmission] {
        &self.admissions
    }

    pub fn assignments(&self) -> &[ComputeAssignment] {
        &self.assignments
    }

    /// Atomically admits the exact resource bindings already sealed into one
    /// planned placement. The owner rechecks them against the capability NEED
    /// and current observations; it never edits or substitutes Plan truth.
    pub fn admit_planned_placement(
        &mut self,
        plan_id: PlanId,
        placement: &crate::PlannedGear,
        observations: &[ResourceObservation],
    ) -> Result<&ResourceAdmission, ResourceAdmissionRefusal> {
        if placement.host_id != self.host.host_id {
            return Err(ResourceAdmissionRefusal::ForeignHost);
        }
        if placement.boot_id != self.host.boot_id
            || placement.offer_generation != self.host.offer_generation
        {
            return Err(ResourceAdmissionRefusal::StaleOffer);
        }
        let capability = self
            .host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == placement.capability_id)
            .ok_or(ResourceAdmissionRefusal::InvalidBinding)?;
        let mut items = Vec::with_capacity(placement.resources.len());
        for binding in &placement.resources {
            let requirement = capability
                .resource_requirements
                .iter()
                .find(|requirement| requirement.class_id == binding.class_id)
                .ok_or(ResourceAdmissionRefusal::InvalidBinding)?;
            items.push(ResourceAdmissionItem {
                requirement: requirement.clone(),
                binding: binding.clone(),
            });
        }
        if items.len() != capability.resource_requirements.len() {
            return Err(ResourceAdmissionRefusal::InvalidBinding);
        }
        let host_id = placement.host_id.clone();
        let boot_id = placement.boot_id.clone();
        self.admit(
            ResourceAdmissionRequest {
                plan_id,
                placement_id: placement.placement_id.clone(),
                host_id,
                boot_id,
                offer_generation: placement.offer_generation,
                items,
            },
            observations,
        )
    }

    pub fn admit(
        &mut self,
        request: ResourceAdmissionRequest,
        observations: &[ResourceObservation],
    ) -> Result<&ResourceAdmission, ResourceAdmissionRefusal> {
        if request.items.is_empty() {
            return Err(ResourceAdmissionRefusal::Empty);
        }
        if request.items.len() > MAXIMUM_BINDINGS_PER_ADMISSION
            || self.admissions.len() == MAXIMUM_RESOURCE_ADMISSIONS
        {
            return Err(ResourceAdmissionRefusal::CapacityExceeded);
        }
        if request.host_id != self.host.host_id {
            return Err(ResourceAdmissionRefusal::ForeignHost);
        }
        if request.boot_id != self.host.boot_id
            || request.offer_generation != self.host.offer_generation
        {
            return Err(ResourceAdmissionRefusal::StaleOffer);
        }
        if self.admissions.iter().any(|admission| {
            admission.plan_id == request.plan_id && admission.placement_id == request.placement_id
        }) {
            return Err(ResourceAdmissionRefusal::DuplicatePlanPlacement);
        }

        let mut signs = Vec::with_capacity(request.items.len());
        let mut pending = Vec::<(&crate::ResourcePoolId, &crate::ResourceClassId, u32)>::new();
        for item in &request.items {
            let pool = self
                .host
                .resources
                .iter()
                .find(|pool| {
                    pool.pool_id == item.binding.pool_id && pool.class_id == item.binding.class_id
                })
                .ok_or(ResourceAdmissionRefusal::InvalidBinding)?;
            if !resource_binding_satisfies(&item.binding, &item.requirement, pool)
                || crate::bind_resource_content(
                    &item.requirement,
                    pool,
                    &self.host.host_id,
                    &self.host.boot_id,
                )
                .is_err()
            {
                return Err(ResourceAdmissionRefusal::InvalidBinding);
            }
            if item.binding.content.as_ref().is_some_and(|content| {
                content.contract.access == crate::ResourceAccessMode::WriteCandidatePublish
            }) && self
                .admissions
                .iter()
                .flat_map(|admission| &admission.items)
                .any(|existing| {
                    existing.binding.pool_id == item.binding.pool_id
                        && existing.binding.content.as_ref().is_some_and(|content| {
                            content.contract.access
                                == crate::ResourceAccessMode::WriteCandidatePublish
                        })
                })
            {
                return Err(ResourceAdmissionRefusal::CompetingResourceWriter);
            }
            let observation = observations
                .iter()
                .find(|observation| {
                    observation.pool_id == pool.pool_id && observation.class_id == pool.class_id
                })
                .ok_or(ResourceAdmissionRefusal::MissingObservation)?;
            if observation.host_id != self.host.host_id
                || observation.boot_id != self.host.boot_id
                || observation.offer_generation != self.host.offer_generation
            {
                return Err(ResourceAdmissionRefusal::StaleObservation);
            }
            if observation.health != ResourceHealth::Ready {
                return Err(ResourceAdmissionRefusal::Unavailable);
            }
            let already = self
                .admissions
                .iter()
                .flat_map(|admission| &admission.items)
                .filter(|reserved| {
                    reserved.binding.pool_id == pool.pool_id
                        && reserved.binding.class_id == pool.class_id
                })
                .try_fold(0u32, |total, reserved| {
                    total.checked_add(reserved.binding.units)
                })
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            let within_request = pending
                .iter()
                .filter(|(pool_id, class_id, _)| {
                    **pool_id == pool.pool_id && **class_id == pool.class_id
                })
                .try_fold(0u32, |total, (_, _, units)| total.checked_add(*units))
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            let total = already
                .checked_add(within_request)
                .and_then(|value| value.checked_add(item.binding.units))
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            if total > observation.unreserved_units || total > pool.capacity_units {
                return Err(ResourceAdmissionRefusal::Overcommitted);
            }
            pending.push((&pool.pool_id, &pool.class_id, item.binding.units));
            signs.push(observation.sign_id.clone());
        }
        signs.sort();
        signs.dedup();
        self.admissions.push(ResourceAdmission {
            plan_id: request.plan_id,
            placement_id: request.placement_id,
            host_id: request.host_id,
            boot_id: request.boot_id,
            offer_generation: request.offer_generation,
            items: request.items,
            observation_sign_ids: signs,
        });
        Ok(self.admissions.last().expect("admission was appended"))
    }

    pub fn release(
        &mut self,
        plan_id: &PlanId,
        placement_id: &PlacementId,
    ) -> Result<ResourceAdmission, ResourceAdmissionRefusal> {
        self.release_for(plan_id, placement_id, ResourceReleaseCause::Aborted)
            .map(|released| released.admission)
    }

    pub fn release_for(
        &mut self,
        plan_id: &PlanId,
        placement_id: &PlacementId,
        cause: ResourceReleaseCause,
    ) -> Result<ReleasedResourceAdmission, ResourceAdmissionRefusal> {
        let index = self
            .admissions
            .iter()
            .position(|item| &item.plan_id == plan_id && &item.placement_id == placement_id)
            .ok_or(ResourceAdmissionRefusal::UnknownAdmission)?;
        self.assignments
            .retain(|item| &item.plan_id != plan_id || &item.placement_id != placement_id);
        Ok(ReleasedResourceAdmission {
            admission: self.admissions.remove(index),
            cause,
        })
    }

    pub fn assign_compute_lanes(
        &mut self,
        plan_id: &PlanId,
        placement_id: &PlacementId,
        active_play_id: ActivePlayId,
        base_lane_ids: &[BaseExecutionLaneId],
    ) -> Result<&ComputeAssignment, ResourceAdmissionRefusal> {
        let admission = self
            .admissions
            .iter()
            .find(|item| &item.plan_id == plan_id && &item.placement_id == placement_id)
            .ok_or(ResourceAdmissionRefusal::ForeignPlayOrPlacement)?;
        let compute = admission
            .items
            .iter()
            .filter_map(|item| item.binding.compute.as_ref())
            .next()
            .ok_or(ResourceAdmissionRefusal::NoComputeEntitlement)?;
        if base_lane_ids.len() > compute.selected_lanes as usize {
            return Err(ResourceAdmissionRefusal::TooManyLanes);
        }
        let unique = base_lane_ids.iter().collect::<BTreeSet<_>>();
        if unique.len() != base_lane_ids.len() {
            return Err(ResourceAdmissionRefusal::DuplicateLane);
        }
        let replacing = self.assignments.iter().position(|item| {
            &item.plan_id == plan_id
                && &item.placement_id == placement_id
                && item.active_play_id == active_play_id
        });
        if replacing.is_none() && self.assignments.len() == MAXIMUM_TRANSIENT_LANE_ASSIGNMENTS {
            return Err(ResourceAdmissionRefusal::AssignmentCapacityExceeded);
        }
        let lanes = base_lane_ids
            .iter()
            .cloned()
            .map(|base_lane_id| ComputeLaneAssignment {
                architecture_base_id: compute.architecture_base_id.clone(),
                base_lane_id,
                active_play_id: active_play_id.clone(),
                placement_id: placement_id.clone(),
            })
            .collect();
        let assignment = ComputeAssignment {
            plan_id: plan_id.clone(),
            placement_id: placement_id.clone(),
            active_play_id,
            lanes,
        };
        let index = match replacing {
            Some(index) => {
                self.assignments[index] = assignment;
                index
            }
            None => {
                self.assignments.push(assignment);
                self.assignments.len() - 1
            }
        };
        Ok(&self.assignments[index])
    }
}
