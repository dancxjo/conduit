use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    ExecutionProfileId, ExecutionRegionId, HostBaseId, PlacementId, PlanFragment, ResourceBinding,
    RUNTIME_MEMORY_RESOURCE_CLASS, TIMER_RESOURCE_CLASS,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionScheduling {
    CooperativeBoundedStep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRegion {
    pub region_id: ExecutionRegionId,
    pub admitted_placements: Vec<PlacementId>,
    pub execution_profile_id: ExecutionProfileId,
    pub scheduling: ExecutionScheduling,
    pub lane_count: u32,
    pub lane_resource: ResourceBinding,
    pub lane_base_id: HostBaseId,
    pub requirements: ExecutionRegionRequirements,
    pub preemption_required: bool,
    pub isolation_required: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRegionRequirements {
    pub runtime_memory_bytes: u32,
    pub timer_slots: u32,
    pub cord_item_capacity: u32,
    pub cord_byte_capacity: u32,
    pub mandatory_sign_items: u16,
    pub mandatory_sign_bytes: u32,
}

pub(crate) fn verify_execution_regions(fragment: &PlanFragment) -> bool {
    let mut region_ids = fragment
        .execution_regions
        .iter()
        .map(|r| &r.region_id)
        .collect::<Vec<_>>();
    region_ids.sort();
    if region_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return false;
    }
    let placement_ids = fragment
        .placements
        .iter()
        .map(|p| &p.placement_id)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    for region in &fragment.execution_regions {
        if region.region_id.as_str().is_empty()
            || region.execution_profile_id.as_str().is_empty()
            || region.admitted_placements.is_empty()
            || region.lane_count == 0
            || region.lane_resource.units != region.lane_count
            || region.lane_resource.class_id.as_str() != "conduit.resource/execution-lane@1"
            || region.lane_base_id.as_str().is_empty()
        {
            return false;
        }
        let Some(compute) = &region.lane_resource.compute else {
            return false;
        };
        if compute.selected_lanes != region.lane_count
            || compute.architecture_base_id.as_str() != region.lane_base_id.as_str()
        {
            return false;
        }
        let resource_total = |class| {
            fragment
                .placements
                .iter()
                .flat_map(|p| &p.resources)
                .filter(|r| r.class_id.as_str() == class)
                .try_fold(0u32, |total, r| total.checked_add(r.units))
        };
        let cord_items = fragment.connections.iter().try_fold(0u32, |total, c| {
            total.checked_add(u32::from(c.item_capacity))
        });
        let cord_bytes = fragment
            .connections
            .iter()
            .try_fold(0u32, |total, c| total.checked_add(c.byte_capacity));
        if resource_total(RUNTIME_MEMORY_RESOURCE_CLASS)
            != Some(region.requirements.runtime_memory_bytes)
            || resource_total(TIMER_RESOURCE_CLASS) != Some(region.requirements.timer_slots)
            || cord_items != Some(region.requirements.cord_item_capacity)
            || cord_bytes != Some(region.requirements.cord_byte_capacity)
            || region.requirements.mandatory_sign_items
                != fragment.sign_storage_budget.item_capacity
            || region.requirements.mandatory_sign_bytes
                != fragment.sign_storage_budget.byte_capacity
        {
            return false;
        }
        if !region
            .admitted_placements
            .windows(2)
            .all(|pair| pair[0] < pair[1])
            || region
                .admitted_placements
                .iter()
                .any(|p| !placement_ids.contains(&p))
        {
            return false;
        }
        admitted.extend(region.admitted_placements.iter());
    }
    admitted.sort();
    !admitted.windows(2).any(|pair| pair[0] == pair[1])
}
