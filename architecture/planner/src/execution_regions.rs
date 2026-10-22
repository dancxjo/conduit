//! Explicit partition realization after ordinary exact placement planning.
//!
//! This seals selection, not availability or a reservation. The target must
//! check current offers/observations, operation owners and full backing before
//! Play. It never chooses threads from authored Source or invents connectivity.
use alloc::vec::Vec;
use conduit_core::*;

pub struct PartitionedRegionSelection {
    pub region_id: ExecutionRegionId,
    pub placements: Vec<PlacementId>,
    pub execution_profile_id: ExecutionProfileId,
    pub lane_resource: ResourceBinding,
    pub lane_base_id: HostBaseId,
    pub worker_storage: ExecutionWorkerStorage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionPlanningError {
    InvalidPlan,
    UnsupportedPlan,
    InvalidPartition,
    MovementIneligible,
    CapacityOverflow,
}

/// Re-seal one local pure-Value Plan with explicit, complete region selection.
/// Original Source/checked/expanded identities, placements and Cords survive;
/// fragment and Plan identities change to commit the exact realization.
///
/// The initial supported profile has one/two/four regions, one lane each, no
/// retained state, external Fore, shared pools, fusion or activation coordinator.
/// Effects retain their existing owner and are refused rather than migrated.
pub fn seal_partitioned_execution(
    mut plan: Plan,
    selections: Vec<PartitionedRegionSelection>,
) -> Result<Plan, RegionPlanningError> {
    use RegionPlanningError::*;
    if !verify_plan(&plan) {
        return Err(InvalidPlan);
    }
    if !matches!(selections.len(), 1 | 2 | 4) || !plan.activations.is_empty() {
        return Err(UnsupportedPlan);
    }
    let [fragment] = plan.fragments.as_mut_slice() else {
        return Err(UnsupportedPlan);
    };
    if !fragment.execution_regions.is_empty()
        || !fragment.states.is_empty()
        || !fragment.fore_ports.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_fusions.is_empty()
    {
        return Err(UnsupportedPlan);
    }
    for placement in &fragment.placements {
        derive_planned_transformation_eligibility(placement)
            .and_then(|facts| facts.require(WorkTransformation::Move))
            .map_err(|_| MovementIneligible)?;
        if placement.base.is_some()
            || placement
                .inputs
                .iter()
                .chain(&placement.outputs)
                .any(|p| p.temporal != PortTemporal::Value)
        {
            return Err(MovementIneligible);
        }
    }
    for selection in selections {
        let mut placements = selection.placements;
        placements.sort();
        if placements.is_empty()
            || placements.windows(2).any(|p| p[0] == p[1])
            || placements
                .iter()
                .any(|id| !fragment.placements.iter().any(|p| &p.placement_id == id))
        {
            return Err(InvalidPartition);
        }
        let resource_total = |class| {
            fragment
                .placements
                .iter()
                .filter(|p| placements.contains(&p.placement_id))
                .flat_map(|p| &p.resources)
                .filter(|r| r.class_id.as_str() == class)
                .try_fold(0u32, |total, r| total.checked_add(r.units))
                .ok_or(CapacityOverflow)
        };
        let incident = || {
            fragment.connections.iter().filter(|c| {
                placements.contains(&c.source_placement_id)
                    || placements.contains(&c.sink_placement_id)
            })
        };
        let requirements = ExecutionRegionRequirements {
            runtime_memory_bytes: resource_total(RUNTIME_MEMORY_RESOURCE_CLASS)?,
            timer_slots: resource_total(TIMER_RESOURCE_CLASS)?,
            cord_item_capacity: incident()
                .try_fold(0u32, |total, c| {
                    total.checked_add(u32::from(c.item_capacity))
                })
                .ok_or(CapacityOverflow)?,
            cord_byte_capacity: incident()
                .try_fold(0u32, |total, c| total.checked_add(c.byte_capacity))
                .ok_or(CapacityOverflow)?,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        };
        fragment.execution_regions.push(ExecutionRegion {
            region_id: selection.region_id,
            admitted_placements: placements,
            execution_profile_id: selection.execution_profile_id,
            scheduling: ExecutionScheduling::PartitionedBoundedStep {
                storage: selection.worker_storage,
            },
            lane_count: 1,
            lane_resource: selection.lane_resource,
            lane_base_id: selection.lane_base_id,
            requirements,
            preemption_required: false,
            isolation_required: false,
        });
    }
    // Preserve cross-region startup order: no worker may race a dependency that
    // the original exact Plan required to happen first.
    for dependency in &fragment.startup_dependencies {
        let region_for = |id| {
            fragment
                .execution_regions
                .iter()
                .position(|r| r.admitted_placements.contains(id))
        };
        if region_for(&dependency.prerequisite_placement_id)
            != region_for(&dependency.dependent_placement_id)
        {
            return Err(UnsupportedPlan);
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
        return Err(InvalidPartition);
    }
    Ok(selected)
}
