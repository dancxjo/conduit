//! Compiler/planner custody shared by configured Masks and input-policy producers.
//! This proves source correspondence, not resource reservation, current binding,
//! active input, or effect/Show acknowledgement.
use alloc::collections::BTreeMap;
use conduit_core::{BaseImplementationId, HostAdvertisement, Plan};
use conduit_planner::{
    plan_expanded_authoring_with_options, ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices,
    PlanningOptions,
};
use conduit_plot::{
    expand_canonical_plot_for_authoring_with_backs, CanonicalBackCatalog, CheckedSyntaxDocument,
    ExpandedAuthoringPlot, ProfileCatalog,
};

pub struct CheckedSourcePlanInputs<'a> {
    pub hosts: &'a [HostAdvertisement],
    pub placements: &'a PlacementChoices,
    pub bases: &'a [BaseImplementationId],
    pub options: PlanningOptions<'a>,
    pub boundary_limits: &'a BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedSourcePlanRefusal {
    SourceExpansion,
    PlanReproduction,
}

pub struct PreparedCheckedSourcePlan<'a> {
    plan: &'a Plan,
    expanded: &'a ExpandedAuthoringPlot,
}
impl PreparedCheckedSourcePlan<'_> {
    pub fn plan(&self) -> &Plan {
        self.plan
    }
    pub fn expanded(&self) -> &ExpandedAuthoringPlot {
        self.expanded
    }
}

pub fn prepare_checked_source_plan<'a>(
    document: &CheckedSyntaxDocument,
    expanded: &'a ExpandedAuthoringPlot,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    plan: &'a Plan,
    selection: CheckedSourcePlanInputs<'_>,
) -> Result<PreparedCheckedSourcePlan<'a>, CheckedSourcePlanRefusal> {
    let reproduced = reproduce_checked_expansion(document, expanded, catalog, backs)?;
    require_selected_source_plan(&reproduced, plan, selection)?;
    Ok(PreparedCheckedSourcePlan { plan, expanded })
}

pub(crate) fn reproduce_checked_expansion(
    document: &CheckedSyntaxDocument,
    expanded: &ExpandedAuthoringPlot,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> Result<ExpandedAuthoringPlot, CheckedSourcePlanRefusal> {
    let reproduced = expand_canonical_plot_for_authoring_with_backs(
        document,
        &expanded.expanded.name,
        catalog,
        backs,
    )
    .map_err(|_| CheckedSourcePlanRefusal::SourceExpansion)?;
    if reproduced != *expanded {
        return Err(CheckedSourcePlanRefusal::SourceExpansion);
    }
    Ok(reproduced)
}

pub(crate) fn require_selected_source_plan(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    selection: CheckedSourcePlanInputs<'_>,
) -> Result<(), CheckedSourcePlanRefusal> {
    let reproduced = plan_expanded_authoring_with_options(
        expanded,
        selection.hosts,
        selection.placements,
        selection.bases,
        selection.options,
        selection.boundary_limits,
    )
    .map_err(|_| CheckedSourcePlanRefusal::PlanReproduction)?;
    if reproduced != *plan {
        return Err(CheckedSourcePlanRefusal::PlanReproduction);
    }
    Ok(())
}
