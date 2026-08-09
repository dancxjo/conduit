use crate::transport_types::*;
use conduit_core::SourceDocumentId;
use patchbay_model::{DistributedRoutePresentation, GraphItemKind, PatchbayPresentation};

pub const SNAPSHOT_SCHEMA: &str = "conduit.patchbay.renderer-snapshot/1";
pub const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
pub const MAX_SNAPSHOT_LINEAR_LINES: usize = 1_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    Oversized,
    Malformed(String),
    UnsupportedSchema,
    Stale { minimum: u64, offered: u64 },
    InvalidIdentity,
    BoundExceeded(&'static str),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oversized => f.write_str("renderer snapshot exceeds its finite byte bound"),
            Self::Malformed(message) => write!(f, "malformed renderer snapshot: {message}"),
            Self::UnsupportedSchema => f.write_str("unsupported renderer snapshot schema"),
            Self::Stale { minimum, offered } => {
                write!(f, "stale renderer revision {offered}; minimum is {minimum}")
            }
            Self::InvalidIdentity => f.write_str("renderer snapshot identity chain is invalid"),
            Self::BoundExceeded(name) => write!(f, "renderer snapshot exceeds {name} bound"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl RendererSnapshot {
    pub fn from_presentation(value: &PatchbayPresentation) -> Self {
        let view = &value.document;
        let document = DocumentSnapshot {
            revision: view.revision,
            path: view.path.to_string_lossy().into_owned(),
            source_document_id: view.checked.source_document_id.clone(),
            open_form: view.open_form.clone(),
            forms: view.checked.forms.iter().map(form_snapshot).collect(),
            diagnostics: view
                .checked
                .diagnostics
                .iter()
                .map(|item| DiagnosticSnapshot {
                    code: item.code.into(),
                    message: item.message.clone(),
                    span: span(item.span),
                })
                .collect(),
            selection: view.selection.as_ref().map(|item| SelectionSnapshot {
                identity: item.identity.clone(),
                span: span(item.span),
            }),
            attempted_edit: value
                .attempted_edit
                .as_ref()
                .map(|attempted| AttemptedEditSnapshot {
                    revision: attempted.revision,
                    source: attempted.source.clone(),
                    diagnostics: attempted
                        .diagnostics
                        .iter()
                        .map(|item| DiagnosticSnapshot {
                            code: item.code.into(),
                            message: item.message.clone(),
                            span: span(item.span),
                        })
                        .collect(),
                }),
        };
        let plan = value.plan.as_ref().map(|item| PlanSnapshot {
            source_document_id: item.source_document_id.clone(),
            checked_form_id: item.checked_form_id.clone(),
            expanded_form_id: item.expanded_form_id.clone(),
            plan_id: item.plan_id.clone(),
            exact: item.exact.clone(),
        });
        let play = value.play.as_ref().map(|item| PlaySnapshot {
            active_play_id: item.active_play_id.clone(),
            plan_id: item.plan_id.clone(),
            terminal: item.terminal,
            evidence: item.evidence.clone(),
        });
        let routes = value.routes.iter().map(route_snapshot).collect::<Vec<_>>();
        let mut linear = vec![format!(
            "FORM source={} checked={}",
            document
                .source_document_id
                .as_ref()
                .map_or("unavailable", SourceDocumentId::as_str),
            document
                .forms
                .iter()
                .find(|form| form.name == document.open_form)
                .map_or("unavailable", |form| form.checked_form_id.as_str())
        )];
        if let Some(document) = &value.plan {
            linear.extend(document.lines.clone());
        }
        if let Some(document) = &value.play {
            linear.extend(document.lines.clone());
        }
        linear.extend(routes.iter().flat_map(|route| route.linear.clone()));
        Self {
            schema: SNAPSHOT_SCHEMA.into(),
            revision: value.revision,
            document,
            plan,
            play,
            topology: value.topology.clone(),
            routes,
            linear,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, SnapshotError> {
        if self.schema != SNAPSHOT_SCHEMA {
            return Err(SnapshotError::UnsupportedSchema);
        }
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], minimum_revision: u64) -> Result<Self, SnapshotError> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if value.schema != SNAPSHOT_SCHEMA {
            return Err(SnapshotError::UnsupportedSchema);
        }
        if value.revision < minimum_revision {
            return Err(SnapshotError::Stale {
                minimum: minimum_revision,
                offered: value.revision,
            });
        }
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), SnapshotError> {
        let graph_items = self.document.forms.iter().fold(0usize, |count, form| {
            count
                .saturating_add(form.items.len())
                .saturating_add(form.cords.len())
                .saturating_add(
                    form.cords
                        .iter()
                        .map(|cord| cord.stages.len())
                        .sum::<usize>(),
                )
        });
        if graph_items > patchbay_model::MAX_RENDERER_GRAPH_ITEMS {
            return Err(SnapshotError::BoundExceeded("graph item"));
        }
        if self.document.diagnostics.len() > patchbay_model::MAX_RENDERER_DIAGNOSTICS {
            return Err(SnapshotError::BoundExceeded("diagnostic"));
        }
        if self
            .document
            .attempted_edit
            .as_ref()
            .is_some_and(|attempted| {
                attempted.revision <= self.document.revision
                    || attempted.source.len() > patchbay_model::MAX_FORM_SOURCE_BYTES
                    || attempted.diagnostics.is_empty()
                    || attempted.diagnostics.len() > patchbay_model::MAX_RENDERER_DIAGNOSTICS
            })
        {
            return Err(SnapshotError::BoundExceeded("attempted edit"));
        }
        if self.routes.len() > patchbay_model::MAX_RENDERER_ROUTES {
            return Err(SnapshotError::BoundExceeded("route"));
        }
        let candidates = self.routes.iter().fold(0usize, |count, route| {
            count
                .saturating_add(route.new_plan.candidates.len())
                .saturating_add(route.same_plan.candidates.len())
        });
        if candidates > patchbay_model::MAX_RENDERER_ROUTE_CANDIDATES {
            return Err(SnapshotError::BoundExceeded("route candidate"));
        }
        let evidence = self
            .play
            .as_ref()
            .map_or(0, |play| play.evidence.len())
            .saturating_add(
                self.topology
                    .as_ref()
                    .map_or(0, |report| report.evidence.len()),
            );
        if evidence > patchbay_model::MAX_RENDERER_EVIDENCE {
            return Err(SnapshotError::BoundExceeded("evidence"));
        }
        if self
            .topology
            .as_ref()
            .is_some_and(|report| report.hosts.len() > patchbay_model::MAX_RENDERER_TOPOLOGY_ITEMS)
        {
            return Err(SnapshotError::BoundExceeded("host"));
        }
        if self.linear.len() > MAX_SNAPSHOT_LINEAR_LINES {
            return Err(SnapshotError::BoundExceeded("linear item"));
        }
        let checked = self
            .document
            .forms
            .iter()
            .find(|form| form.name == self.document.open_form)
            .map(|form| &form.checked_form_id);
        if self.plan.as_ref().is_some_and(|plan| {
            Some(&plan.source_document_id) != self.document.source_document_id.as_ref()
                || Some(&plan.checked_form_id) != checked
                || plan.exact.plan_id != plan.plan_id
                || plan.exact.source_document_id != plan.source_document_id
                || plan.exact.checked_form_id != plan.checked_form_id
                || plan.exact.expanded_form_id != plan.expanded_form_id
                || !conduit_core::verify_plan(&plan.exact)
        }) || self.play.as_ref().is_some_and(|play| {
            self.plan.as_ref().map(|plan| &plan.plan_id) != Some(&play.plan_id)
                || play.evidence.iter().any(|observation| {
                    observation
                        .plan_id
                        .as_ref()
                        .is_some_and(|identity| identity != &play.plan_id)
                        || observation
                            .active_play_id
                            .as_ref()
                            .is_some_and(|identity| identity != &play.active_play_id)
                })
        }) {
            return Err(SnapshotError::InvalidIdentity);
        }
        Ok(())
    }
}

fn span(value: conduit_form::Span) -> SpanSnapshot {
    SpanSnapshot {
        start: value.start,
        end: value.end,
        line: value.line,
        column: value.column,
        end_line: value.end_line,
        end_column: value.end_column,
    }
}

fn form_snapshot(value: &patchbay_model::GraphForm) -> FormSnapshot {
    FormSnapshot {
        name: value.name.clone(),
        checked_form_id: value.checked_form_id.clone(),
        items: value
            .items
            .iter()
            .map(|item| GraphItemSnapshot {
                identity: item.identity.clone(),
                label: item.label.clone(),
                kind: match item.kind {
                    GraphItemKind::FaceInput => GraphSubjectKind::FaceInput,
                    GraphItemKind::FaceOutput => GraphSubjectKind::FaceOutput,
                    GraphItemKind::StartupValue => GraphSubjectKind::StartupValue,
                    GraphItemKind::Cell => GraphSubjectKind::Cell,
                    GraphItemKind::Cord => GraphSubjectKind::Cord,
                },
                span: span(item.source_span),
            })
            .collect(),
        cords: value
            .cords
            .iter()
            .map(|cord| CordSnapshot {
                identity: cord.identity.clone(),
                stages: cord
                    .stages
                    .iter()
                    .map(|stage| match stage {
                        patchbay_model::GraphCordStage::Reference(name) => {
                            CordStageSnapshot::Reference(name.clone())
                        }
                        patchbay_model::GraphCordStage::InlineCell { operation } => {
                            CordStageSnapshot::InlineCell {
                                operation: operation.clone(),
                            }
                        }
                        patchbay_model::GraphCordStage::Literal => CordStageSnapshot::Literal,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn candidates(value: &[patchbay_model::RouteCandidatePresentation]) -> Vec<CandidateSnapshot> {
    value
        .iter()
        .map(|item| CandidateSnapshot {
            order: item.order,
            binding_id: item.binding_id.clone(),
            provider: item.provider,
            provider_instance_id: item.provider_instance_id.clone(),
        })
        .collect()
}

fn route_snapshot(value: &DistributedRoutePresentation) -> RouteSnapshot {
    RouteSnapshot {
        source_document_id: value.source_document_id.clone(),
        checked_form_id: value.checked_form_id.clone(),
        new_plan: RecoverySnapshot {
            prior_plan_id: value.new_plan.prior.plan_id.clone(),
            replacement_plan_id: value.new_plan.replacement_plan_id.clone(),
            connection_id: value.new_plan.prior.connection_id.clone(),
            candidates: candidates(&value.new_plan.prior.candidates),
            unavailable_binding_id: value.new_plan.unavailable_binding_id.clone(),
            evidence_ids: vec![
                value.new_plan.unavailable_evidence_id.clone(),
                value.new_plan.unsatisfied_evidence_id.clone(),
                value.new_plan.planning_request_evidence_id.clone(),
                value.new_plan.planning_success_evidence_id.clone(),
                value.new_plan.installed_evidence_id.clone(),
            ],
        },
        same_plan: FallbackSnapshot {
            plan_id: value.same_plan.plan.plan_id.clone(),
            connection_id: value.same_plan.plan.connection_id.clone(),
            candidates: candidates(&value.same_plan.plan.candidates),
            unavailable_binding_id: value.same_plan.unavailable_binding_id.clone(),
            selected_binding_id: value.same_plan.selected_binding_id.clone(),
            evidence_ids: vec![
                value.same_plan.unavailable_evidence_id.clone(),
                value.same_plan.selection_evidence_id.clone(),
            ],
        },
        refused_binding_id: value.refused.binding_id.clone(),
        refused_evidence_id: value.refused.observation_evidence_id.clone(),
        linear: value.linear_lines(),
    }
}
