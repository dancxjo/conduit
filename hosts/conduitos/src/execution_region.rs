use alloc::{vec, vec::Vec};
use conduit_core::{
    ArchitectureBaseId, ArchitectureBaseKind, ComputeReservation, ComputeServiceGuarantee,
    ExecutionProfileId, ExecutionRegion, ExecutionRegionId, ExecutionRegionRequirements,
    ExecutionScheduling, FormIdentity, HostAdvertisement, HostBaseId, Plan, ResourceBinding,
    seal_plan,
};

use crate::{
    machine::BaseKind,
    offer::{BaseOffer, HostOffer},
    ordinary_plan::{COOPERATIVE_REGION_PROFILE, PreparationError},
};

pub(super) fn seal_execution_region(
    plan: Plan,
    advertisement: &HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<Plan, PreparationError> {
    if fixed.profile != "conduitos/single-lane-cooperative@1" || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let lane_base = exact_lane_base(fixed)?;
    let lane_offer = exact_lane_offer(advertisement)?;
    let lane_base_identity = hex_identity(&lane_base.id);
    let mut fragments = plan.fragments;
    let fragment = &mut fragments[0];
    let cord_item_capacity = fragment
        .connections
        .iter()
        .try_fold(0u32, |total, c| {
            total.checked_add(u32::from(c.item_capacity))
        })
        .ok_or(PreparationError::PlanRejected)?;
    let cord_byte_capacity = fragment
        .connections
        .iter()
        .try_fold(0u32, |total, c| total.checked_add(c.byte_capacity))
        .ok_or(PreparationError::PlanRejected)?;
    let mut admitted_placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    admitted_placements.sort();
    fragment.execution_regions = vec![ExecutionRegion {
        region_id: ExecutionRegionId::from("region/0"),
        admitted_placements,
        execution_profile_id: ExecutionProfileId::from(COOPERATIVE_REGION_PROFILE),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: 1,
        lane_resource: ResourceBinding {
            pool_id: lane_offer.pool_id.clone(),
            class_id: lane_offer.class_id.clone(),
            units: 1,
            protected: None,
            compute: Some(ComputeReservation {
                selected_lanes: 1,
                service_guarantee: ComputeServiceGuarantee::Reserved,
                architecture_base_id: ArchitectureBaseId::from(lane_base_identity.clone()),
                architecture_base_kind: ArchitectureBaseKind::BareMetal,
                topology_group_id: None,
            }),
        },
        lane_base_id: HostBaseId::from(lane_base_identity),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: resource_total(
                fragment,
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            )?,
            timer_slots: resource_total(fragment, conduit_core::TIMER_RESOURCE_CLASS)?,
            cord_item_capacity,
            cord_byte_capacity,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        },
        preemption_required: false,
        isolation_required: false,
    }];
    Ok(seal_plan(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        fragments,
    ))
}

pub(super) fn validate_execution_region(
    fragment: &conduit_core::PlanFragment,
    advertisement: &HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    let [region] = fragment.execution_regions.as_slice() else {
        return Err(PreparationError::PlanRejected);
    };
    let lane_base = exact_lane_base(fixed)?;
    let lane_offer = exact_lane_offer(advertisement)?;
    let mut placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    placements.sort();
    if region.region_id.as_str() != "region/0"
        || region.admitted_placements != placements
        || region.execution_profile_id.as_str() != COOPERATIVE_REGION_PROFILE
        || region.scheduling != ExecutionScheduling::CooperativeBoundedStep
        || region.lane_count != 1
        || region.lane_resource.pool_id != lane_offer.pool_id
        || region.lane_resource.class_id != lane_offer.class_id
        || region.lane_resource.units != 1
        || region.lane_base_id.as_str() != hex_identity(&lane_base.id)
        || region.preemption_required
        || region.isolation_required
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn exact_lane_base<'a>(fixed: &'a HostOffer<'_>) -> Result<&'a BaseOffer, PreparationError> {
    fixed
        .bases
        .iter()
        .find(|base| base.kind == BaseKind::ExecutionLane && base.capacity == 1)
        .ok_or(PreparationError::PlanRejected)
}

fn exact_lane_offer(
    advertisement: &HostAdvertisement,
) -> Result<&conduit_core::ResourceOffer, PreparationError> {
    advertisement
        .resources
        .iter()
        .find(|resource| {
            resource.class_id.as_str() == "conduit.resource/execution-lane@1"
                && resource.capacity_units == 1
        })
        .ok_or(PreparationError::PlanRejected)
}

fn resource_total(
    fragment: &conduit_core::PlanFragment,
    class: &str,
) -> Result<u32, PreparationError> {
    fragment
        .placements
        .iter()
        .flat_map(|placement| &placement.resources)
        .filter(|resource| resource.class_id.as_str() == class)
        .try_fold(0u32, |total, resource| total.checked_add(resource.units))
        .ok_or(PreparationError::PlanRejected)
}

fn hex_identity(bytes: &[u8; 32]) -> alloc::string::String {
    use core::fmt::Write;
    let mut result = alloc::string::String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}
