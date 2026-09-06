use alloc::{vec, vec::Vec};
use conduit_core::{
    ArchitectureBaseId, ArchitectureBaseKind, ComputeReservation, ComputeServiceGuarantee,
    ExecutionProfileId, ExecutionRegion, ExecutionRegionId, ExecutionRegionRequirements,
    ExecutionScheduling, FormIdentity, HostAdvertisement, HostBaseId, PlacementId, Plan,
    PlanFragment, ResourceBinding, seal_plan,
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
    if fixed.profile != "conduitos/two-lane-cooperative@1" || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let lane_base = exact_lane_base(fixed)?;
    let lane_offers = exact_lane_offers(advertisement)?;
    let lane_base_identity = hex_identity(&lane_base.id);
    let mut fragments = plan.fragments;
    let fragment = &mut fragments[0];
    let mut admitted_placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    admitted_placements.sort();
    fragment.execution_regions = vec![build_region(
        "region/0",
        admitted_placements,
        lane_offers[0],
        lane_base_identity,
        fragment,
    )?];
    Ok(seal_plan(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        fragments,
    ))
}

pub(super) fn seal_two_execution_regions(
    plan: Plan,
    advertisement: &HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<Plan, PreparationError> {
    if fixed.profile != "conduitos/two-lane-cooperative@1" || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let lane_base_identity = hex_identity(&exact_lane_base(fixed)?.id);
    let lane_offers = exact_lane_offers(advertisement)?;
    let mut fragments = plan.fragments;
    let fragment = &mut fragments[0];
    let (text, timer) = branch_placements(fragment)?;
    fragment.execution_regions = vec![
        build_region(
            "region/text",
            text,
            lane_offers[0],
            lane_base_identity.clone(),
            fragment,
        )?,
        build_region(
            "region/timer",
            timer,
            lane_offers[1],
            lane_base_identity,
            fragment,
        )?,
    ];
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
    if advertisement.host_id.as_str() != hex_identity(&fixed.host_id)
        || advertisement.boot_id.as_str() != hex_identity(&fixed.boot_id)
        || advertisement.offer_generation.0 != fixed.generation
        || fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
    {
        return Err(PreparationError::PlanRejected);
    }
    let [region] = fragment.execution_regions.as_slice() else {
        return Err(PreparationError::PlanRejected);
    };
    let lane_base = exact_lane_base(fixed)?;
    let lane_offers = exact_lane_offers(advertisement)?;
    let mut placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    placements.sort();
    let expected = build_region(
        "region/0",
        placements,
        lane_offers[0],
        hex_identity(&lane_base.id),
        fragment,
    )?;
    if region != &expected {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

pub(super) fn validate_two_execution_regions(
    fragment: &PlanFragment,
    advertisement: &HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    validate_fragment_basis(fragment, advertisement, fixed)?;
    let [text_region, timer_region] = fragment.execution_regions.as_slice() else {
        return Err(PreparationError::PlanRejected);
    };
    let lane_base_identity = hex_identity(&exact_lane_base(fixed)?.id);
    let lane_offers = exact_lane_offers(advertisement)?;
    let (text, timer) = branch_placements(fragment)?;
    let expected_text = build_region(
        "region/text",
        text,
        lane_offers[0],
        lane_base_identity.clone(),
        fragment,
    )?;
    let expected_timer = build_region(
        "region/timer",
        timer,
        lane_offers[1],
        lane_base_identity,
        fragment,
    )?;
    if text_region != &expected_text
        || timer_region != &expected_timer
        || text_region.lane_resource.pool_id == timer_region.lane_resource.pool_id
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn validate_fragment_basis(
    fragment: &PlanFragment,
    advertisement: &HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    if advertisement.host_id.as_str() != hex_identity(&fixed.host_id)
        || advertisement.boot_id.as_str() != hex_identity(&fixed.boot_id)
        || advertisement.offer_generation.0 != fixed.generation
        || fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn exact_lane_base<'a>(fixed: &'a HostOffer<'_>) -> Result<&'a BaseOffer, PreparationError> {
    fixed
        .bases
        .iter()
        .find(|base| base.kind == BaseKind::ExecutionLane && base.capacity == 2)
        .ok_or(PreparationError::PlanRejected)
}

fn exact_lane_offers(
    advertisement: &HostAdvertisement,
) -> Result<[&conduit_core::ResourceOffer; 2], PreparationError> {
    let offers = advertisement
        .resources
        .iter()
        .filter(|resource| {
            resource.class_id.as_str() == "conduit.resource/execution-lane@1"
                && resource.capacity_units == 1
        })
        .collect::<Vec<_>>();
    let [first, second] = offers.as_slice() else {
        return Err(PreparationError::PlanRejected);
    };
    if first.pool_id == second.pool_id {
        return Err(PreparationError::PlanRejected);
    }
    Ok([*first, *second])
}

fn resource_total(
    fragment: &PlanFragment,
    placements: &[PlacementId],
    class: &str,
) -> Result<u32, PreparationError> {
    fragment
        .placements
        .iter()
        .filter(|placement| placements.contains(&placement.placement_id))
        .flat_map(|placement| &placement.resources)
        .filter(|resource| resource.class_id.as_str() == class)
        .try_fold(0u32, |total, resource| total.checked_add(resource.units))
        .ok_or(PreparationError::PlanRejected)
}

fn build_region(
    region_id: &str,
    admitted_placements: Vec<PlacementId>,
    lane_offer: &conduit_core::ResourceOffer,
    lane_base_identity: alloc::string::String,
    fragment: &PlanFragment,
) -> Result<ExecutionRegion, PreparationError> {
    let cord_item_capacity = region_connections(fragment, &admitted_placements)
        .try_fold(0u32, |total, connection| {
            total.checked_add(u32::from(connection.item_capacity))
        })
        .ok_or(PreparationError::PlanRejected)?;
    let cord_byte_capacity = region_connections(fragment, &admitted_placements)
        .try_fold(0u32, |total, connection| {
            total.checked_add(connection.byte_capacity)
        })
        .ok_or(PreparationError::PlanRejected)?;
    Ok(ExecutionRegion {
        region_id: ExecutionRegionId::from(region_id),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: resource_total(
                fragment,
                &admitted_placements,
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            )?,
            timer_slots: resource_total(
                fragment,
                &admitted_placements,
                conduit_core::TIMER_RESOURCE_CLASS,
            )?,
            cord_item_capacity,
            cord_byte_capacity,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        },
        admitted_placements,
        execution_profile_id: ExecutionProfileId::from(COOPERATIVE_REGION_PROFILE),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: 1,
        lane_resource: ResourceBinding {
            content: None,
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
                performance_class: None,
                nominal_clock_hz: None,
            }),
        },
        lane_base_id: HostBaseId::from(lane_base_identity),
        preemption_required: false,
        isolation_required: false,
    })
}

fn region_connections<'a>(
    fragment: &'a PlanFragment,
    placements: &'a [PlacementId],
) -> impl Iterator<Item = &'a conduit_core::PlannedConnection> {
    fragment.connections.iter().filter(|connection| {
        placements.contains(&connection.source_placement_id)
            && placements.contains(&connection.sink_placement_id)
    })
}

fn branch_placements(
    fragment: &PlanFragment,
) -> Result<(Vec<PlacementId>, Vec<PlacementId>), PreparationError> {
    let mut text = Vec::new();
    let mut timer = Vec::new();
    for placement in &fragment.placements {
        if matches!(
            placement.kind_id.as_str(),
            conduit_text::TEXT_LITERAL_KIND
                | conduit_text::TEXT_UPPER_KIND
                | conduit_semantic_catalog::TEXT_PRESENTATION_KIND
        ) {
            text.push(placement.placement_id.clone());
        } else if matches!(
            placement.kind_id.as_str(),
            conduit_semantic_catalog::TICK_KIND | conduit_semantic_catalog::TICK_PRESENTATION_KIND
        ) {
            timer.push(placement.placement_id.clone());
        } else {
            return Err(PreparationError::PlanRejected);
        }
    }
    text.sort();
    timer.sort();
    if text.len() != 3 || timer.len() != 2 {
        return Err(PreparationError::PlanRejected);
    }
    Ok((text, timer))
}

fn hex_identity(bytes: &[u8; 32]) -> alloc::string::String {
    use core::fmt::Write;
    let mut result = alloc::string::String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}
