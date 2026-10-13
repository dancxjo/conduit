use crate::prelude::*;
use crate::{
    default_placements_unvalidated, plan_validated_plot, ConnectionEndpoints,
    ConnectionQueueLimits, PlacementChoices, PlannerError, PlanningOptions,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    BaseImplementationId, HostAdvertisement, Plan, PlannedForePort, PlannedGear, PlotIdentity,
};
use conduit_plot::{
    expand_canonical_plot, expand_canonical_plot_for_authoring_with_backs,
    expand_canonical_plot_with_backs, CanonicalBackCatalog, CheckedPlot, CheckedSyntaxDocument,
    ExpandedAuthoringPlot, ExpandedCanonicalPlot, ProfileCatalog,
};

mod activations;
pub use activations::plan_expanded_canonical_with_activations;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeBoundaryKey {
    pub direction: conduit_core::PortDirection,
    pub front_port_id: conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
}

impl PartialOrd for ForeBoundaryKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ForeBoundaryKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.direction as u8, &self.front_port_id, self.track).cmp(&(
            other.direction as u8,
            &other.front_port_id,
            other.track,
        ))
    }
}

mod authoring;
pub use authoring::{
    plan_expanded_authoring_with_activations, plan_expanded_authoring_with_connection_limits,
    plan_expanded_authoring_with_options,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalRealizationMode {
    Direct,
    RecursiveBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCanonicalRealization {
    pub mode: CanonicalRealizationMode,
    pub expanded: ExpandedCanonicalPlot,
    pub placements: PlacementChoices,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRealizationSelectionError {
    InvalidDirectExpansion(String),
    InvalidRecursiveExpansion {
        direct: PlannerError,
        recursive: String,
    },
    NoRealizablePath {
        direct: PlannerError,
        recursive: PlannerError,
    },
}

/// Plans between peer realizations of one checked caller without changing its
/// source. A fully admitted direct Plan wins; otherwise the same checked
/// caller is expanded through the admitted exact Back catalog and planned
/// normally. Mere capability availability cannot suppress a valid Back.
#[allow(clippy::too_many_arguments)]
pub fn plan_canonical_realization_with_options(
    document: &CheckedSyntaxDocument,
    plot_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<PlannedCanonicalRealization, CanonicalRealizationSelectionError> {
    let direct = expand_canonical_plot(document, plot_name, catalog).map_err(|error| {
        CanonicalRealizationSelectionError::InvalidDirectExpansion(error.to_string())
    })?;
    match plan_default_candidate(direct, hosts, bases, options) {
        Ok((expanded, placements, plan)) => Ok(PlannedCanonicalRealization {
            mode: CanonicalRealizationMode::Direct,
            expanded,
            placements,
            plan,
        }),
        Err(direct_error) => {
            let recursive = expand_canonical_plot_with_backs(document, plot_name, catalog, backs)
                .map_err(|error| {
                CanonicalRealizationSelectionError::InvalidRecursiveExpansion {
                    direct: direct_error.clone(),
                    recursive: error.to_string(),
                }
            })?;
            let (expanded, placements, plan) =
                plan_default_candidate(recursive, hosts, bases, options).map_err(
                    |recursive_error| CanonicalRealizationSelectionError::NoRealizablePath {
                        direct: direct_error,
                        recursive: recursive_error,
                    },
                )?;
            Ok(PlannedCanonicalRealization {
                mode: CanonicalRealizationMode::RecursiveBack,
                expanded,
                placements,
                plan,
            })
        }
    }
}

fn plan_default_candidate(
    expanded: ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<(ExpandedCanonicalPlot, PlacementChoices, Plan), PlannerError> {
    let placements = default_expanded_placements(&expanded, hosts)?;
    let plan = plan_expanded_canonical_with_options(&expanded, hosts, &placements, bases, options)?;
    Ok((expanded, placements, plan))
}

pub fn default_expanded_placements(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    plot.validate_expansion()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    default_placements_unvalidated(&plot.gears, hosts)
}

mod default_queues;
pub use default_queues::plan_expanded_canonical;

pub fn plan_expanded_canonical_with_options(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    plot.validate_expansion()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    let planning_plot = CheckedPlot {
        source_document_id: plot.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
        expanded_plot_id: plot.expanded_plot_id.clone(),
        name: plot.name.clone(),
        completion: plot.completion,
        gears: plot.gears.clone(),
        connections: plot.connections.clone(),
        exports: Vec::new(),
        nested_plots: Vec::new(),
    };
    let mut plan = plan_validated_plot(&planning_plot, hosts, placements, bases, options)?;
    attach_source_spans(plot, &mut plan)?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::PlotIdentity {
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
            },
            crate::plan_completion_policy(plot.completion),
            plot.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

pub fn plan_expanded_canonical_with_connection_limits(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    plot.validate_expansion()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    let mut plan = crate::validated_planning::plan_borrowed_plot_with_connection_limits(
        crate::planning_input::PlanningInput::from(plot),
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    attach_source_spans(plot, &mut plan)?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::PlotIdentity {
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
            },
            crate::plan_completion_policy(plot.completion),
            plot.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

fn attach_source_spans(plot: &ExpandedCanonicalPlot, plan: &mut Plan) -> Result<(), PlannerError> {
    for placement in plan
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
    {
        let provenance = plot
            .provenance
            .iter()
            .find(|entry| entry.gear_id == placement.gear_id.as_str())
            .ok_or_else(|| {
                PlannerError::InvalidPlotIdentity(format!(
                    "expanded Gear '{}' has no exact source provenance",
                    placement.gear_id.as_str()
                ))
            })?;
        let span = provenance.source_span;
        let span = conduit_core::SourceSpan {
            start: span.start.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span start exceeds u64".into())
            })?,
            end: span.end.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span end exceeds u64".into())
            })?,
            line: span.line.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span line exceeds u64".into())
            })?,
            column: span.column.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span column exceeds u64".into())
            })?,
            end_line: span.end_line.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span end line exceeds u64".into())
            })?,
            end_column: span.end_column.try_into().map_err(|_| {
                PlannerError::InvalidPlotIdentity("source span end column exceeds u64".into())
            })?,
        };
        if !span.is_valid() {
            return Err(PlannerError::InvalidPlotIdentity(
                "expanded Gear source span is invalid".into(),
            ));
        }
        placement.source_span = Some(span);
    }
    Ok(())
}

mod shared_pools;
pub use shared_pools::{
    plan_expanded_canonical_with_shared_pool_policies, plan_expanded_canonical_with_shared_pools,
    SharedPoolPlanningRequirement, SharedPoolPolicyPlanningError, SharedPoolPolicySelection,
    MAXIMUM_SHARED_POOL_POLICY_SELECTIONS,
};
