//! Finite JSON transport shapes for the browser delivery adapter.
//!
//! These are not Conduit's portable Presentation/Manifestation semantics.

use conduit_core::{
    ActivePlayId, CheckedFormId, ConnectionProvider, EvidenceId, ExpandedFormId, Observation, Plan,
    PlanId, SourceDocumentId, TerminalDisposition,
};
use conduit_observatory::ObservatoryReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSnapshot {
    pub schema: String,
    pub revision: u64,
    pub document: DocumentSnapshot,
    pub plan: Option<PlanSnapshot>,
    pub play: Option<PlaySnapshot>,
    pub topology: Option<ObservatoryReport>,
    pub routes: Vec<RouteSnapshot>,
    pub linear: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSnapshot {
    pub revision: u64,
    pub path: String,
    pub source_document_id: Option<SourceDocumentId>,
    pub open_form: String,
    pub forms: Vec<FormSnapshot>,
    pub diagnostics: Vec<DiagnosticSnapshot>,
    pub selection: Option<SelectionSnapshot>,
    pub attempted_edit: Option<AttemptedEditSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptedEditSnapshot {
    pub revision: u64,
    pub source: String,
    pub diagnostics: Vec<DiagnosticSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormSnapshot {
    pub name: String,
    pub checked_form_id: CheckedFormId,
    pub items: Vec<GraphItemSnapshot>,
    pub cords: Vec<CordSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CordSnapshot {
    pub identity: String,
    pub stages: Vec<CordStageSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CordStageSnapshot {
    Reference(String),
    InlineCell { operation: String },
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphSubjectKind {
    FaceInput,
    FaceOutput,
    StartupValue,
    Cell,
    Cord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphItemSnapshot {
    pub identity: String,
    pub label: String,
    pub kind: GraphSubjectKind,
    pub span: SpanSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanSnapshot {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticSnapshot {
    pub code: String,
    pub message: String,
    pub span: SpanSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub identity: String,
    pub span: SpanSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSnapshot {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
    pub exact: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaySnapshot {
    pub active_play_id: ActivePlayId,
    pub plan_id: PlanId,
    pub terminal: TerminalDisposition,
    pub evidence: Vec<Observation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSnapshot {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub new_plan: RecoverySnapshot,
    pub same_plan: FallbackSnapshot,
    pub refused_binding_id: conduit_core::LinkBindingId,
    pub refused_evidence_id: EvidenceId,
    pub linear: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySnapshot {
    pub prior_plan_id: PlanId,
    pub replacement_plan_id: PlanId,
    pub connection_id: conduit_core::ConnectionId,
    pub candidates: Vec<CandidateSnapshot>,
    pub unavailable_binding_id: conduit_core::LinkBindingId,
    pub evidence_ids: Vec<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackSnapshot {
    pub plan_id: PlanId,
    pub connection_id: conduit_core::ConnectionId,
    pub candidates: Vec<CandidateSnapshot>,
    pub unavailable_binding_id: conduit_core::LinkBindingId,
    pub selected_binding_id: conduit_core::LinkBindingId,
    pub evidence_ids: Vec<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSnapshot {
    pub order: usize,
    pub binding_id: conduit_core::LinkBindingId,
    pub provider: ConnectionProvider,
    pub provider_instance_id: conduit_core::ConnectionProviderInstanceId,
}
