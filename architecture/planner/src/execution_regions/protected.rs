//! Exact protected policy selection from existing current Host resource offers.
//! This commits realization; it does not reserve backing or install preemption.
use alloc::vec::Vec;
use conduit_core::*;

pub struct ProtectedRegionSelection {
    pub region_id: ExecutionRegionId,
    pub placements: Vec<PlacementId>,
    pub execution_profile_id: ExecutionProfileId,
    pub lane_resource: ResourceBinding,
    pub lane_base_id: HostBaseId,
    pub context_binding: ResourceBinding,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedRegionPlanningError {
    InvalidPlan,
    UnsupportedPlan,
    StaleHost,
    InvalidRegion,
    MissingPool,
    AmbiguousPool,
    UnsupportedProfile,
    BindingMismatch,
    CapacityExceeded,
    InvalidBase,
}

/// Seal local protected regions without changing authored Source, checked Plot,
/// Back ownership, effects or connectivity. Policy and context requirements are
/// copied from the exact offered lane pool, never supplied by the caller.
/// The target must independently admit current authority and complete backing
/// before Play; existing targets explicitly refuse this unsupported profile.
pub fn seal_protected_preemptible_execution(
    mut plan: Plan,
    host: &HostAdvertisement,
    selections: Vec<ProtectedRegionSelection>,
) -> Result<Plan, ProtectedRegionPlanningError> {
    use ProtectedRegionPlanningError::*;
    if !verify_plan(&plan) {
        return Err(InvalidPlan);
    }
    let [fragment] = plan.fragments.as_mut_slice() else {
        return Err(UnsupportedPlan);
    };
    if !fragment.execution_regions.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !plan.activations.is_empty()
        || selections.is_empty()
    {
        return Err(UnsupportedPlan);
    }
    if host.protocol_version != PROTOCOL_VERSION
        || fragment.host_id != host.host_id
        || fragment.boot_id != host.boot_id
        || fragment.offer_generation != host.offer_generation
    {
        return Err(StaleHost);
    }
    for selection in selections {
        let lane_offer = unique_offer(host, &selection.lane_resource.pool_id)?;
        let contract = lane_offer.compute.as_ref().ok_or(UnsupportedProfile)?;
        let offered = contract
            .protected_preemption
            .as_ref()
            .ok_or(UnsupportedProfile)?;
        if !contract.is_valid_for_capacity(lane_offer.capacity_units)
            || offered.execution_profile_id != selection.execution_profile_id
        {
            return Err(UnsupportedProfile);
        }
        validate_lane(&selection, lane_offer, host)?;
        let context_offer = unique_offer(host, &selection.context_binding.pool_id)?;
        let context = ExecutionContinuationStorage {
            binding: selection.context_binding,
            bytes: offered.context_bytes,
            alignment: offered.context_alignment,
        };
        if !context.is_valid()
            || context_offer.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
            || context_offer.compute.is_some()
            || context_offer.content.is_some()
        {
            return Err(BindingMismatch);
        }
        let mut placements = selection.placements;
        placements.sort();
        if selection.region_id.as_str().is_empty()
            || placements.is_empty()
            || placements.windows(2).any(|p| p[0] == p[1])
            || placements
                .iter()
                .any(|id| !fragment.placements.iter().any(|p| &p.placement_id == id))
        {
            return Err(InvalidRegion);
        }
        let requirements = requirements(fragment, &placements)?;
        fragment.execution_regions.push(ExecutionRegion {
            region_id: selection.region_id,
            admitted_placements: placements,
            execution_profile_id: offered.execution_profile_id.clone(),
            scheduling: ExecutionScheduling::ProtectedPreemptibleStep {
                policy: offered.policy,
                context,
            },
            lane_count: 1,
            lane_resource: selection.lane_resource,
            lane_base_id: selection.lane_base_id,
            requirements,
            preemption_required: true,
            isolation_required: true,
        });
    }
    // Include existing placement demand: additional continuation/lane storage
    // cannot silently spend a pool already consumed by the original Plan.
    let bindings: Vec<_> = fragment
        .placements
        .iter()
        .flat_map(|p| &p.resources)
        .chain(fragment.execution_regions.iter().flat_map(|r| {
            let ExecutionScheduling::ProtectedPreemptibleStep { context, .. } = &r.scheduling
            else {
                unreachable!()
            };
            [&r.lane_resource, &context.binding]
        }))
        .collect();
    for region in &fragment.execution_regions {
        let ExecutionScheduling::ProtectedPreemptibleStep { context, .. } = &region.scheduling
        else {
            unreachable!()
        };
        for binding in [&region.lane_resource, &context.binding] {
            let offer = unique_offer(host, &binding.pool_id)?;
            let total = bindings
                .iter()
                .filter(|b| b.pool_id == binding.pool_id)
                .try_fold(0u32, |total, b| {
                    if b.class_id != offer.class_id {
                        return None;
                    }
                    total.checked_add(b.units)
                })
                .ok_or(CapacityExceeded)?;
            if total > offer.capacity_units {
                return Err(CapacityExceeded);
            }
        }
    }
    let selected = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.activations,
        plan.fragments,
    );
    if !verify_plan(&selected) {
        return Err(InvalidRegion);
    }
    Ok(selected)
}

fn unique_offer<'a>(
    host: &'a HostAdvertisement,
    pool: &ResourcePoolId,
) -> Result<&'a ResourceOffer, ProtectedRegionPlanningError> {
    let mut offers = host.resources.iter().filter(|o| &o.pool_id == pool);
    let offer = offers
        .next()
        .ok_or(ProtectedRegionPlanningError::MissingPool)?;
    if offers.next().is_some() {
        return Err(ProtectedRegionPlanningError::AmbiguousPool);
    }
    Ok(offer)
}

fn validate_lane(
    selection: &ProtectedRegionSelection,
    offer: &ResourceOffer,
    host: &HostAdvertisement,
) -> Result<(), ProtectedRegionPlanningError> {
    use ProtectedRegionPlanningError::*;
    let binding = &selection.lane_resource;
    let compute = binding.compute.as_ref().ok_or(BindingMismatch)?;
    let requirement = ResourceRequirement {
        class_id: "conduit.resource/execution-lane@1".into(),
        units: 1,
        protected_role: None,
        content: None,
        compute: Some(ComputeRequirement {
            minimum_lanes: 1,
            preferred_lanes: 1,
            maximum_lanes: 1,
            minimum_service_guarantee: compute.service_guarantee,
            topology: compute
                .topology_group_id
                .as_ref()
                .map(|_| ComputeTopologyRequirement {
                    same_numa_domain: false,
                    same_cache_domain: false,
                    performance_class: compute.performance_class.clone(),
                }),
        }),
    };
    if binding.units != 1
        || compute.selected_lanes != 1
        || compute.architecture_base_kind != ArchitectureBaseKind::BareMetal
        || compute.architecture_base_id.as_str() != selection.lane_base_id.as_str()
    {
        return Err(BindingMismatch);
    }
    if binding.units > offer.capacity_units {
        return Err(CapacityExceeded);
    }
    if !resource_binding_satisfies(binding, &requirement, offer) {
        return Err(BindingMismatch);
    }
    let mut owners = host
        .bases
        .iter()
        .filter(|base| base.resource_pool_ids.contains(&offer.pool_id));
    let base = owners.next().ok_or(InvalidBase)?;
    if owners.next().is_some()
        || base.base_id != selection.lane_base_id
        || base.lifecycle != BaseLifecycle::Ready
        || base.enforcement_class != BaseEnforcementClass::ConduitOsKernelEnforced
        || base.provider_generation == 0
        || base.provider_instance_id.as_str().is_empty()
        || base.implementation_id.as_str().is_empty()
        || base.mechanism_family.as_str().is_empty()
    {
        return Err(InvalidBase);
    }
    Ok(())
}

fn requirements(
    fragment: &PlanFragment,
    placements: &[PlacementId],
) -> Result<ExecutionRegionRequirements, ProtectedRegionPlanningError> {
    use ProtectedRegionPlanningError::CapacityExceeded;
    let resources = |class| {
        fragment
            .placements
            .iter()
            .filter(|p| placements.contains(&p.placement_id))
            .flat_map(|p| &p.resources)
            .filter(|r| r.class_id.as_str() == class)
            .try_fold(0u32, |n, r| n.checked_add(r.units))
            .ok_or(CapacityExceeded)
    };
    let connections = || {
        fragment.connections.iter().filter(|c| {
            placements.contains(&c.source_placement_id) && placements.contains(&c.sink_placement_id)
        })
    };
    Ok(ExecutionRegionRequirements {
        runtime_memory_bytes: resources(RUNTIME_MEMORY_RESOURCE_CLASS)?,
        timer_slots: resources(TIMER_RESOURCE_CLASS)?,
        cord_item_capacity: connections()
            .try_fold(0u32, |n, c| n.checked_add(u32::from(c.item_capacity)))
            .ok_or(CapacityExceeded)?,
        cord_byte_capacity: connections()
            .try_fold(0u32, |n, c| n.checked_add(c.byte_capacity))
            .ok_or(CapacityExceeded)?,
        mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
        mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
    })
}
