//! Seal the selected physical choice without changing logical Plan contracts.
use super::*;
use crate::plan_with_options;
use conduit_core::{BaseImplementationId, FusionId, PlannedFusion};

pub fn plan_selected_optimization(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    selection: &FusionSelection,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<OptimizedPlan, PlannerError> {
    if selection.checked_plot_id != plot.checked_plot_id {
        return Err(PlannerError::InvalidPlotIdentity(
            "fusion selection belongs to a different checked plot".to_string(),
        ));
    }
    let plan = plan_with_options(
        plot,
        hosts,
        &selection.selected_realization.placements,
        bases,
        options,
    )?;
    attach_selected_fusions(plan, selection)
}

/// Preserve expanded-source spans and external Fores while selecting the same
/// physical alternatives as the ordinary checked-graph fusion planner.
#[allow(clippy::too_many_arguments)]
pub fn plan_expanded_selected_optimization(
    plot: &conduit_plot::ExpandedAuthoringPlot,
    hosts: &[HostAdvertisement],
    selection: &FusionSelection,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    boundary_limits: &alloc::collections::BTreeMap<
        crate::ForeBoundaryKey,
        crate::ConnectionQueueLimits,
    >,
) -> Result<OptimizedPlan, PlannerError> {
    if selection.checked_plot_id != plot.expanded.checked_plot_id {
        return Err(PlannerError::InvalidPlotIdentity(
            "fusion selection belongs to a different checked plot".into(),
        ));
    }
    let plan = crate::plan_expanded_authoring_with_options(
        plot,
        hosts,
        &selection.selected_realization.placements,
        bases,
        options,
        boundary_limits,
    )?;
    attach_selected_fusions(plan, selection)
}

fn attach_selected_fusions(
    mut plan: conduit_core::Plan,
    selection: &FusionSelection,
) -> Result<OptimizedPlan, PlannerError> {
    for group in &selection.selected_fusion_groups {
        let fragment = plan
            .fragments
            .iter_mut()
            .find(|fragment| fragment.host_id == group.host_id && fragment.boot_id == group.boot_id)
            .ok_or_else(|| {
                PlannerError::InvalidPlanningObservation(
                    "fusion Host has no matching Plan fragment".to_string(),
                )
            })?;
        let mut preserved_placements = group
            .preserved_gear_ids
            .iter()
            .map(|gear_id| {
                fragment
                    .placements
                    .iter()
                    .find(|placement| &placement.gear_id == gear_id)
                    .map(|placement| placement.placement_id.clone())
                    .ok_or_else(|| {
                        PlannerError::InvalidPlanningObservation(
                            "fusion Gear has no exact plan placement".to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        preserved_placements.sort();
        let mut preserved_connections = group
            .preserved_cords
            .iter()
            .map(|(source_gear, sink_gear)| {
                let source = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.gear_id == *source_gear)
                    .expect("fusion Gear placement was collected");
                let sink = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.gear_id == *sink_gear)
                    .expect("fusion Gear placement was collected");
                fragment
                    .connections
                    .iter()
                    .find(|connection| {
                        connection.source_placement_id == source.placement_id
                            && connection.sink_placement_id == sink.placement_id
                    })
                    .map(|connection| connection.connection_id.clone())
                    .ok_or_else(|| {
                        PlannerError::InvalidPlanningObservation(
                            "fusion Cord has no exact plan connection".to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        preserved_connections.sort();
        fragment.execution_fusions.push(PlannedFusion {
            fusion_id: FusionId::from(group.fusion_id.clone()),
            execution_profile_id: group.execution_profile_id.clone(),
            implementation_id: group.implementation_id.clone(),
            artifact_id: group.artifact_id.clone(),
            preserved_placements,
            preserved_connections,
        });
        fragment
            .execution_fusions
            .sort_by(|left, right| left.fusion_id.cmp(&right.fusion_id));
    }
    plan = conduit_core::seal_plan_with_realization_backs_and_completion(
        conduit_core::PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.completion_policy,
        plan.realization_backs.clone(),
        plan.fragments,
    );
    let optimized = OptimizedPlan {
        plan,
        fusion_groups: selection.selected_fusion_groups.clone(),
    };
    if !optimized.verify() {
        return Err(PlannerError::InvalidPlanningObservation(
            "fusion explanation does not match the ordinary plan".to_string(),
        ));
    }
    Ok(optimized)
}

/// Select over the validated expanded graph without losing authored identity.
pub fn select_expanded_fusion_candidate(
    plot: &conduit_plot::ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    candidates: &[FusionCandidate],
    locality_basis: &LocalityPlanningBasis,
    inputs: FusionPlanningInputs<'_>,
) -> Result<FusionSelection, PlannerError> {
    plot.validate_expansion()
        .map_err(|e| PlannerError::InvalidPlotIdentity(e.to_string()))?;
    super::select_fusion_candidate_validated(
        &crate::canonical::expanded_planning_plot(plot),
        hosts,
        candidates,
        locality_basis,
        inputs,
    )
}
