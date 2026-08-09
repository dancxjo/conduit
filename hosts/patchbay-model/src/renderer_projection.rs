//! Finite, toolkit-independent facts shared by Patchbay renderers.

use crate::{
    DistributedRoutePresentation, EditorDiagnostic, FormDocumentView, PlanDocument, PlayDocument,
    SourceSelection,
};
use conduit_core::{
    verify_plan, ActivePlayId, CheckedFormId, EvidenceId, ExpandedFormId, PlanId, SourceDocumentId,
};
use conduit_observatory::ObservatoryReport;

pub const MAX_RENDERER_GRAPH_ITEMS: usize = 512;
pub const MAX_RENDERER_DIAGNOSTICS: usize = 128;
pub const MAX_RENDERER_EVIDENCE: usize = 512;
pub const MAX_RENDERER_ROUTES: usize = 32;
pub const MAX_RENDERER_ROUTE_CANDIDATES: usize = 512;
pub const MAX_RENDERER_TOPOLOGY_ITEMS: usize = 1_024;
pub const MAX_RENDERER_INSPECTION_LINES: usize = 512;
pub const MAX_RENDERER_PLAN_ITEMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererProjectionError {
    GraphTooLarge,
    TooManyDiagnostics,
    TooMuchEvidence,
    TooManyRoutes,
    TooManyRouteCandidates,
    TopologyTooLarge,
    InspectionTooLarge,
    PlanTooLarge,
    SourceTooLarge,
    OpenFormMissing,
    IdentityMismatch,
    InvalidAttemptedEdit,
}

impl std::fmt::Display for RendererProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GraphTooLarge => formatter.write_str("renderer graph exceeds its finite bound"),
            Self::TooManyDiagnostics => {
                formatter.write_str("renderer diagnostics exceed their finite bound")
            }
            Self::TooMuchEvidence => {
                formatter.write_str("renderer evidence exceeds its finite bound")
            }
            Self::TooManyRoutes => formatter.write_str("renderer routes exceed their finite bound"),
            Self::TooManyRouteCandidates => {
                formatter.write_str("renderer route candidates exceed their finite bound")
            }
            Self::TopologyTooLarge => {
                formatter.write_str("renderer topology exceeds its finite bound")
            }
            Self::InspectionTooLarge => {
                formatter.write_str("renderer inspection text exceeds its finite line bound")
            }
            Self::PlanTooLarge => {
                formatter.write_str("renderer Plan exceeds its finite item bound")
            }
            Self::SourceTooLarge => {
                formatter.write_str("renderer source exceeds its finite byte bound")
            }
            Self::OpenFormMissing => {
                formatter.write_str("renderer document does not contain its open Form")
            }
            Self::IdentityMismatch => {
                formatter.write_str("renderer inputs do not describe one exact identity chain")
            }
            Self::InvalidAttemptedEdit => {
                formatter.write_str("renderer attempted edit is stale, empty, or unbounded")
            }
        }
    }
}

impl std::error::Error for RendererProjectionError {}

/// Exact identities whose truth must not depend on renderer-local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererIdentityProjection {
    pub source_document_id: Option<SourceDocumentId>,
    pub document_checked_form_id: Option<CheckedFormId>,
    pub plan_checked_form_id: Option<CheckedFormId>,
    pub plan_id: Option<PlanId>,
    pub expanded_form_id: Option<ExpandedFormId>,
    pub active_play_id: Option<ActivePlayId>,
    pub evidence_ids: Vec<EvidenceId>,
}

/// One immutable revision of the ordinary Patchbay presentation surface.
///
/// Renderers may retain their own finite layout, viewport, theme, disclosure,
/// and focus state. None of that state enters this projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayPresentation {
    pub revision: u64,
    pub document: FormDocumentView,
    pub plan: Option<PlanDocument>,
    pub play: Option<PlayDocument>,
    pub topology: Option<ObservatoryReport>,
    pub routes: Vec<DistributedRoutePresentation>,
    pub attempted_edit: Option<AttemptedEditPresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptedEditPresentation {
    pub revision: u64,
    pub source: String,
    pub diagnostics: Vec<EditorDiagnostic>,
}

impl PatchbayPresentation {
    pub fn new(
        revision: u64,
        document: FormDocumentView,
        plan: Option<PlanDocument>,
        play: Option<PlayDocument>,
        topology: Option<ObservatoryReport>,
        routes: Vec<DistributedRoutePresentation>,
    ) -> Result<Self, RendererProjectionError> {
        let graph_items = document
            .checked
            .forms
            .iter()
            .try_fold(0usize, |count, form| {
                count
                    .checked_add(form.items.len())?
                    .checked_add(form.cords.len())?
                    .checked_add(
                        form.cords
                            .iter()
                            .map(|cord| cord.stages.len())
                            .sum::<usize>(),
                    )
            })
            .ok_or(RendererProjectionError::GraphTooLarge)?;
        if graph_items > MAX_RENDERER_GRAPH_ITEMS {
            return Err(RendererProjectionError::GraphTooLarge);
        }
        if document.checked.diagnostics.len() > MAX_RENDERER_DIAGNOSTICS {
            return Err(RendererProjectionError::TooManyDiagnostics);
        }
        if document.source.len() > crate::MAX_FORM_SOURCE_BYTES {
            return Err(RendererProjectionError::SourceTooLarge);
        }
        let report_evidence = topology.as_ref().map_or(0, |report| report.evidence.len());
        let play_evidence = play.as_ref().map_or(0, |play| play.evidence.len());
        if report_evidence.saturating_add(play_evidence) > MAX_RENDERER_EVIDENCE {
            return Err(RendererProjectionError::TooMuchEvidence);
        }
        if routes.len() > MAX_RENDERER_ROUTES {
            return Err(RendererProjectionError::TooManyRoutes);
        }
        let route_candidates = routes.iter().fold(0usize, |count, route| {
            count
                .saturating_add(route.new_plan.prior.candidates.len())
                .saturating_add(route.same_plan.plan.candidates.len())
        });
        if route_candidates > MAX_RENDERER_ROUTE_CANDIDATES {
            return Err(RendererProjectionError::TooManyRouteCandidates);
        }
        if topology
            .as_ref()
            .is_some_and(|report| topology_item_count(report) > MAX_RENDERER_TOPOLOGY_ITEMS)
        {
            return Err(RendererProjectionError::TopologyTooLarge);
        }
        if plan
            .as_ref()
            .is_some_and(|document| document.lines.len() > MAX_RENDERER_INSPECTION_LINES)
            || play
                .as_ref()
                .is_some_and(|document| document.lines.len() > MAX_RENDERER_INSPECTION_LINES)
        {
            return Err(RendererProjectionError::InspectionTooLarge);
        }
        if plan
            .as_ref()
            .is_some_and(|document| plan_item_count(&document.exact) > MAX_RENDERER_PLAN_ITEMS)
        {
            return Err(RendererProjectionError::PlanTooLarge);
        }
        if !document.open_form.is_empty()
            && !document
                .checked
                .forms
                .iter()
                .any(|form| form.name == document.open_form)
        {
            return Err(RendererProjectionError::OpenFormMissing);
        }
        if plan.as_ref().is_some_and(|plan| {
            Some(&plan.source_document_id) != document.checked.source_document_id.as_ref()
                || !document
                    .checked
                    .forms
                    .iter()
                    .any(|form| form.checked_form_id == plan.checked_form_id)
                || plan.exact.plan_id != plan.plan_id
                || plan.exact.source_document_id != plan.source_document_id
                || plan.exact.checked_form_id != plan.checked_form_id
                || plan.exact.expanded_form_id != plan.expanded_form_id
                || !verify_plan(&plan.exact)
        }) || play.as_ref().is_some_and(|play| {
            plan.as_ref().map(|plan| &plan.plan_id) != Some(&play.plan_id)
                || play.evidence.iter().any(|observation| {
                    observation
                        .plan_id
                        .as_ref()
                        .is_some_and(|id| id != &play.plan_id)
                        || observation
                            .active_play_id
                            .as_ref()
                            .is_some_and(|id| id != &play.active_play_id)
                })
        }) {
            return Err(RendererProjectionError::IdentityMismatch);
        }
        Ok(Self {
            revision,
            document,
            plan,
            play,
            topology,
            routes,
            attempted_edit: None,
        })
    }

    pub fn with_attempted_edit(
        mut self,
        attempted: AttemptedEditPresentation,
    ) -> Result<Self, RendererProjectionError> {
        if attempted.revision <= self.document.revision
            || attempted.source.len() > crate::MAX_FORM_SOURCE_BYTES
            || attempted.diagnostics.is_empty()
            || attempted.diagnostics.len() > MAX_RENDERER_DIAGNOSTICS
        {
            return Err(RendererProjectionError::InvalidAttemptedEdit);
        }
        self.attempted_edit = Some(attempted);
        Ok(self)
    }

    pub fn selection(&self) -> Option<&SourceSelection> {
        self.document.selection.as_ref()
    }

    pub fn identities(&self) -> RendererIdentityProjection {
        let document_checked_form_id = self
            .document
            .checked
            .forms
            .iter()
            .find(|form| form.name == self.document.open_form)
            .map(|form| form.checked_form_id.clone());
        let source_document_id = self.document.checked.source_document_id.clone();
        let plan_id = self.plan.as_ref().map(|plan| plan.plan_id.clone());
        let plan_checked_form_id = self.plan.as_ref().map(|plan| plan.checked_form_id.clone());
        let expanded_form_id = self.plan.as_ref().map(|plan| plan.expanded_form_id.clone());
        let active_play_id = self.play.as_ref().map(|play| play.active_play_id.clone());
        let mut evidence_ids = self.play.as_ref().map_or_else(Vec::new, |play| {
            play.evidence
                .iter()
                .map(|observation| observation.evidence_id.clone())
                .collect()
        });
        if let Some(report) = &self.topology {
            evidence_ids.extend(report.evidence.iter().map(|row| row.evidence_id.clone()));
        }
        Self::deduplicate_evidence(&mut evidence_ids);
        RendererIdentityProjection {
            source_document_id,
            document_checked_form_id,
            plan_checked_form_id,
            plan_id,
            expanded_form_id,
            active_play_id,
            evidence_ids,
        }
    }

    fn deduplicate_evidence(evidence_ids: &mut Vec<EvidenceId>) {
        evidence_ids.sort();
        evidence_ids.dedup();
    }
}

fn topology_item_count(report: &ObservatoryReport) -> usize {
    let top_level = report
        .hosts
        .len()
        .saturating_add(report.capabilities.len())
        .saturating_add(report.links.len())
        .saturating_add(report.plans.len())
        .saturating_add(report.fragments.len())
        .saturating_add(report.placements.len())
        .saturating_add(report.connections.len())
        .saturating_add(report.plays.len())
        .saturating_add(report.evidence.len());
    let host_details = report.hosts.iter().fold(0usize, |count, host| {
        count
            .saturating_add(host.planner_capabilities.len())
            .saturating_add(host.resources.len())
    });
    let capability_details = report
        .capabilities
        .iter()
        .fold(0usize, |count, capability| {
            count
                .saturating_add(capability.inputs.len())
                .saturating_add(capability.outputs.len())
                .saturating_add(capability.host_operations.len())
                .saturating_add(capability.resource_requirements.len())
                .saturating_add(capability.authority_requirements.len())
        });
    let placement_details = report.placements.iter().fold(0usize, |count, placement| {
        count
            .saturating_add(placement.host_operations.len())
            .saturating_add(placement.resources.len())
            .saturating_add(placement.authority.len())
    });
    let play_details = report.plays.iter().fold(0usize, |count, play| {
        count
            .saturating_add(play.placements.len())
            .saturating_add(play.connections.len())
    });
    top_level
        .saturating_add(host_details)
        .saturating_add(capability_details)
        .saturating_add(placement_details)
        .saturating_add(play_details)
}

fn plan_item_count(plan: &conduit_core::Plan) -> usize {
    plan.fragments
        .iter()
        .fold(plan.fragments.len(), |count, fragment| {
            let placement_details = fragment.placements.iter().fold(0usize, |items, placement| {
                items
                    .saturating_add(1)
                    .saturating_add(placement.configuration.len())
                    .saturating_add(placement.realization_characteristics.len())
                    .saturating_add(placement.inputs.len())
                    .saturating_add(placement.outputs.len())
                    .saturating_add(placement.host_operations.len())
                    .saturating_add(placement.resources.len())
                    .saturating_add(placement.authority.len())
                    .saturating_add(placement.pool_references.len())
            });
            let connection_details =
                fragment
                    .connections
                    .iter()
                    .fold(0usize, |items, connection| {
                        items
                            .saturating_add(1)
                            .saturating_add(connection.route_candidates.len())
                    });
            count
                .saturating_add(placement_details)
                .saturating_add(connection_details)
                .saturating_add(fragment.shared_pools.len())
                .saturating_add(fragment.startup_dependencies.len())
                .saturating_add(fragment.startup_order.len())
                .saturating_add(fragment.expected_terminals.len())
                .saturating_add(fragment.expected_evidence.len())
                .saturating_add(fragment.plan_fragments.len())
        })
}

#[cfg(test)]
#[path = "renderer_projection_tests.rs"]
mod tests;
