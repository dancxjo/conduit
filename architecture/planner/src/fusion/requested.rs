//! Explicit realization choice with the same semantic admission as cost search.
//! This path makes no comparison or measured-work claim and creates no synthetic
//! observations. It is the fusion analogue of an explicit placement choice.
use super::*;
use crate::{ConnectionEndpoints, ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices};
use alloc::collections::BTreeMap;
use conduit_core::BaseImplementationId;

#[derive(Clone, Copy)]
pub struct RequestedFusionInputs<'a> {
    pub offers: &'a [FusionRealizationOffer],
    pub semantic_kinds: &'a [conduit_core::Kind],
    pub boundaries: &'a [FusionBoundary],
}

/// Seal only the explicitly requested current-Host alternatives. Every selected
/// member keeps its exact Back, typed ports, Cords and source provenance. This
/// admission does not assert that the choice is cheaper; measured automatic
/// comparison remains owned by `select_expanded_fusion_candidate`.
#[allow(clippy::too_many_arguments)]
pub fn plan_expanded_with_requested_fusions(
    plot: &conduit_plot::ExpandedAuthoringPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
    inputs: RequestedFusionInputs<'_>,
) -> Result<OptimizedPlan, PlannerError> {
    plot.expanded
        .validate_expansion()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    let checked = crate::canonical::expanded_planning_plot(&plot.expanded);
    let groups = admit_requested_groups(&checked, hosts, placements, inputs)?;
    let plan = crate::plan_expanded_authoring_with_connection_limits(
        plot,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
        boundary_limits,
    )?;
    planning::attach_fusion_groups(plan, &groups)
}

/// Explicit checked-graph counterpart. No comparative performance evidence is
/// created or implied by this requested physical choice.
pub fn plan_with_requested_fusions(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    inputs: RequestedFusionInputs<'_>,
) -> Result<OptimizedPlan, PlannerError> {
    plot.validate_identities()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    let groups = admit_requested_groups(plot, hosts, placements, inputs)?;
    let plan = crate::plan_with_options(plot, hosts, placements, bases, options)?;
    planning::attach_fusion_groups(plan, &groups)
}
fn admit_requested_groups(
    checked: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    inputs: RequestedFusionInputs<'_>,
) -> Result<Vec<FusionDecisionGroup>, PlannerError> {
    if inputs.offers.len() > MAXIMUM_FUSION_OFFERS
        || inputs.offers.iter().any(|offer| {
            offer.fusion_id.is_empty()
                || offer.gear_ids.len() > MAXIMUM_FUSION_MEMBERS
                || offer.internal_cords.len() > MAXIMUM_FUSION_MEMBERS
        })
        || inputs
            .offers
            .iter()
            .map(|offer| &offer.fusion_id)
            .collect::<BTreeSet<_>>()
            .len()
            != inputs.offers.len()
    {
        return invalid("requested fusion identities or finite offer bounds are invalid");
    }
    let candidate = FusionCandidate {
        candidate_id: "explicit-realization".into(),
        realization: crate::LocalityCandidate {
            candidate_id: "explicit-realization".into(),
            placements: placements.clone(),
            lines: BTreeMap::new(),
        },
        fusion_ids: inputs
            .offers
            .iter()
            .map(|offer| offer.fusion_id.clone())
            .collect(),
    };
    let mut admitted = BTreeSet::new();
    let mut groups = Vec::with_capacity(inputs.offers.len());
    for offer in inputs.offers {
        validate_fusion_offer(
            checked,
            hosts,
            &candidate,
            offer,
            inputs.semantic_kinds,
            inputs.boundaries,
            &mut admitted,
        )
        .map_err(PlannerError::InvalidPlanningObservation)?;
        groups.push(FusionDecisionGroup {
            fusion_id: offer.fusion_id.clone(),
            host_id: offer.host_id.clone(),
            boot_id: offer.boot_id.clone(),
            execution_profile_id: offer.execution_profile_id.clone(),
            implementation_id: offer.implementation_id.clone(),
            artifact_id: offer.artifact_id.clone(),
            preserved_gear_ids: offer.gear_ids.clone(),
            preserved_cords: offer.internal_cords.clone(),
        });
    }
    Ok(groups)
}
