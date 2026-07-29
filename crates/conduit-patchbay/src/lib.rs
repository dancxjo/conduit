//! Transport-neutral Patchbay authoring and observation protocol.
//!
//! This crate intentionally owns only mutable authoring and presentation
//! projections.  It never makes layout part of `.panel` semantics, resolves a
//! plan, executes a node, or appends executor evidence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const PATCHBAY_PROTOCOL_V1: u16 = 1;

/// Rebuildable presentation of one exact pool generation. Source, plan, run,
/// evidence, and presentation identities remain separate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolProjection {
    pub source_semantic_hash: String,
    pub plan_identity: String,
    pub plan_epoch: u64,
    pub run_id: String,
    pub evidence_stream_id: String,
    pub evidence_cursor: u64,
    pub pool: String,
    pub template_identity: String,
    pub generation_identity: String,
    pub generation: u32,
    pub maximum_live: u16,
    pub maximum_queued: u16,
    pub queued: u16,
    pub live: u16,
    pub restarting: u16,
    pub retiring: u16,
    pub cleanup: u16,
    pub terminal: u16,
    pub latest_evidence: Option<PoolEvidenceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolEvidenceProjection {
    pub sequence: u64,
    pub tick: u64,
    pub instance_identity: String,
    pub work_unit_identity: String,
    pub attempt: u16,
    pub correlation_identity: String,
    pub from: String,
    pub to: String,
    pub reason: String,
    pub cause: Option<String>,
}

/// Exact origins and observations used to rebuild a pool presentation. This
/// input is a view only; it neither owns nor mutates execution state.
pub struct PoolProjectionInput<'a, 'contract> {
    pub source_semantic_hash: &'a str,
    pub plan_identity: conduit_core::SemanticHash,
    pub plan_epoch: u64,
    pub run_id: &'a str,
    pub evidence_stream_id: &'a str,
    pub generation: conduit_core::PoolGeneration,
    pub generation_identity: conduit_core::SemanticHash,
    pub contract: conduit_core::PoolContract<'contract>,
    pub population: conduit_core::PoolPopulationSnapshot,
    pub evidence: &'a [conduit_core::PoolEvidence],
}

#[must_use]
pub fn project_pool(input: PoolProjectionInput<'_, '_>) -> PoolProjection {
    PoolProjection {
        source_semantic_hash: input.source_semantic_hash.to_owned(),
        plan_identity: input.plan_identity.to_string(),
        plan_epoch: input.plan_epoch,
        run_id: input.run_id.to_owned(),
        evidence_stream_id: input.evidence_stream_id.to_owned(),
        evidence_cursor: input.evidence.last().map_or(0, |event| event.sequence),
        pool: input.contract.pool.as_str().to_owned(),
        template_identity: input.contract.template_hash.to_string(),
        generation_identity: input.generation_identity.to_string(),
        generation: input.generation.generation,
        maximum_live: input.contract.maximum_live,
        maximum_queued: input.contract.maximum_queued,
        queued: input.population.queued,
        live: input.population.live,
        restarting: input.population.restarting,
        retiring: input.population.retiring,
        cleanup: input.population.cleanup,
        terminal: input.population.terminal,
        latest_evidence: input.evidence.last().map(|event| PoolEvidenceProjection {
            sequence: event.sequence,
            tick: event.tick,
            instance_identity: event.identity.instance.to_string(),
            work_unit_identity: event.identity.work_unit.to_string(),
            attempt: event.identity.attempt,
            correlation_identity: event.identity.correlation.to_string(),
            from: pool_state_name(event.from).to_owned(),
            to: pool_state_name(event.to).to_owned(),
            reason: event.reason.as_str().to_owned(),
            cause: event.cause.map(|cause| cause.to_string()),
        }),
    }
}

const fn pool_state_name(value: conduit_core::PoolSlotState) -> &'static str {
    match value {
        conduit_core::PoolSlotState::Empty => "empty",
        conduit_core::PoolSlotState::Queued => "queued",
        conduit_core::PoolSlotState::Reserved => "reserved",
        conduit_core::PoolSlotState::Running => "running",
        conduit_core::PoolSlotState::Checkpointing => "checkpointing",
        conduit_core::PoolSlotState::RestartBackoff => "restart-backoff",
        conduit_core::PoolSlotState::Draining => "draining",
        conduit_core::PoolSlotState::Cleanup => "cleanup",
        conduit_core::PoolSlotState::Succeeded => "succeeded",
        conduit_core::PoolSlotState::Cancelled => "cancelled",
        conduit_core::PoolSlotState::Failed => "failed",
    }
}

/// Presentation projection of the portable supervision contract. Every
/// identity remains pinned to its source, plan, run, binding, and evidence
/// origin; this view does not become execution evidence itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SupervisionProjection {
    pub source_semantic_hash: String,
    pub plan_identity: String,
    pub plan_epoch: u64,
    pub run_id: String,
    pub evidence_stream_id: String,
    pub evidence_cursor: u64,
    pub evidence_gap_resume_at: Option<u64>,
    pub semantic_subject: String,
    pub expanded_subject: String,
    pub handler: String,
    pub boundary_id: String,
    pub scope: String,
    pub failure_mode: String,
    pub terminal_class: String,
    pub terminal_cause: String,
    pub terminal_phase: String,
    pub generation: u32,
    pub attempt: u16,
    pub retry: String,
    pub resource: Option<String>,
    pub host: Option<String>,
    pub artifact: Option<String>,
    pub remaining_observations: u16,
    pub remaining_decisions: u16,
    pub remaining_attempts: u16,
    pub remaining_evidence_events: u16,
    pub actions: Vec<SupervisionActionProjection>,
    pub latest_evidence: Option<SupervisionEvidenceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SupervisionActionProjection {
    pub action_index: u16,
    pub kind: String,
    pub target: Option<String>,
    pub maximum_uses: u16,
    pub requires_new_epoch: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SupervisionEvidenceProjection {
    pub sequence: u64,
    pub kind: String,
    pub action_index: Option<u16>,
    pub rejection_code: Option<String>,
}

/// Build a rebuildable Patchbay view from the exact portable values.
#[must_use]
pub fn project_supervision(
    source_semantic_hash: &str,
    observation: conduit_core::TerminalObservation<'_>,
    contract: conduit_core::SupervisionContract<'_>,
    evidence: &[conduit_core::SupervisionEvidence],
    cursor_status: conduit_core::EvidenceCursorStatus,
) -> SupervisionProjection {
    use conduit_core::{
        RetryDeclaration, SupervisionEvidenceKind, SupervisionFailureMode, SupervisionScope,
        TerminalClass, TerminalPhase,
    };
    let actions = contract
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            Some(SupervisionActionProjection {
                action_index: u16::try_from(index).ok()?,
                kind: supervision_action_name(action.kind).to_owned(),
                target: action.target.map(|target| target.to_string()),
                maximum_uses: action.maximum_uses,
                requires_new_epoch: action.requires_new_epoch,
            })
        })
        .collect();
    let latest_evidence = evidence.last().map(|item| SupervisionEvidenceProjection {
        sequence: item.sequence,
        kind: match item.kind {
            SupervisionEvidenceKind::TerminalObserved => "terminal-observed",
            SupervisionEvidenceKind::ObservationAdmitted => "observation-admitted",
            SupervisionEvidenceKind::DecisionAccepted => "decision-accepted",
            SupervisionEvidenceKind::DecisionRejected => "decision-rejected",
            SupervisionEvidenceKind::AttemptStarted => "attempt-started",
            SupervisionEvidenceKind::FallbackSelected => "fallback-selected",
            SupervisionEvidenceKind::DegradedSelected => "degraded-selected",
            SupervisionEvidenceKind::OperatorActionRequested => "operator-action-requested",
            SupervisionEvidenceKind::Exhausted => "exhausted",
            SupervisionEvidenceKind::Propagated => "propagated",
            SupervisionEvidenceKind::CleanupStarted => "cleanup-started",
            SupervisionEvidenceKind::CleanupFailed => "cleanup-failed",
            SupervisionEvidenceKind::Cancelled => "cancelled",
            SupervisionEvidenceKind::HandlerFailed => "handler-failed",
            SupervisionEvidenceKind::FinalOutcome => "final-outcome",
        }
        .to_owned(),
        action_index: item.action_index,
        rejection_code: item.reason.map(|reason| reason.code().to_owned()),
    });
    SupervisionProjection {
        source_semantic_hash: source_semantic_hash.to_owned(),
        plan_identity: observation.plan_identity.to_string(),
        plan_epoch: observation.plan_epoch,
        run_id: observation.run.to_string(),
        evidence_stream_id: observation.evidence.stream.to_string(),
        evidence_cursor: observation.evidence.sequence,
        evidence_gap_resume_at: match cursor_status {
            conduit_core::EvidenceCursorStatus::Gap { resume_at } => Some(resume_at),
            conduit_core::EvidenceCursorStatus::Available
            | conduit_core::EvidenceCursorStatus::Future { .. } => None,
        },
        semantic_subject: observation.semantic_subject.as_str().to_owned(),
        expanded_subject: observation.expanded_subject.as_str().to_owned(),
        handler: contract.handler.as_str().to_owned(),
        boundary_id: contract.id.to_string(),
        scope: match contract.scope {
            SupervisionScope::Child => "child",
            SupervisionScope::NamedGroup => "named-group",
            SupervisionScope::CompositeBoundary => "composite-boundary",
            SupervisionScope::ReplicatedChild => "replicated-child",
        }
        .to_owned(),
        failure_mode: match contract.failure_mode {
            SupervisionFailureMode::FailTogether => "fail-together",
            SupervisionFailureMode::IsolatedOptional => "isolated-optional",
        }
        .to_owned(),
        terminal_class: match observation.class {
            TerminalClass::Succeeded => "succeeded",
            TerminalClass::Cancelled => "cancelled",
            TerminalClass::Failed => "failed",
            TerminalClass::Disconnected => "disconnected",
        }
        .to_owned(),
        terminal_cause: observation.code.as_str().to_owned(),
        terminal_phase: match observation.phase {
            TerminalPhase::Prepare => "prepare",
            TerminalPhase::Start => "start",
            TerminalPhase::Step => "step",
            TerminalPhase::HostOperation => "host-operation",
            TerminalPhase::Drain => "drain",
            TerminalPhase::Cleanup => "cleanup",
        }
        .to_owned(),
        generation: observation.generation,
        attempt: observation.attempt,
        retry: match observation.retry {
            RetryDeclaration::Undeclared => "undeclared",
            RetryDeclaration::Idempotent => "idempotent",
            RetryDeclaration::RestartOnly => "restart-only",
        }
        .to_owned(),
        resource: observation.context.resource.map(|value| value.to_string()),
        host: observation.context.host.map(|value| value.to_string()),
        artifact: observation.context.artifact.map(|value| value.to_string()),
        remaining_observations: observation.budget.remaining_observations,
        remaining_decisions: observation.budget.remaining_decisions,
        remaining_attempts: observation.budget.remaining_attempts,
        remaining_evidence_events: observation.budget.remaining_evidence_events,
        actions,
        latest_evidence,
    }
}

fn supervision_action_name(kind: conduit_core::SupervisionActionKind) -> &'static str {
    match kind {
        conduit_core::SupervisionActionKind::Propagate => "propagate",
        conduit_core::SupervisionActionKind::StopScope => "stop-scope",
        conduit_core::SupervisionActionKind::RestartSame => "restart-same",
        conduit_core::SupervisionActionKind::RetrySame => "retry-same",
        conduit_core::SupervisionActionKind::ActivateDeclaredFallback => {
            "activate-declared-fallback"
        }
        conduit_core::SupervisionActionKind::ContinueDeclaredDegradedMode => {
            "continue-declared-degraded-mode"
        }
        conduit_core::SupervisionActionKind::RequestOperatorAction => "request-operator-action",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub document_id: String,
    pub revision: u64,
    pub source: String,
    pub semantic_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: i32,
    pub y: i32,
}

/// Presentation-only state. Its identity deliberately excludes source,
/// descriptor, plan, run, and evidence identities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresentationSnapshot {
    pub document_id: String,
    pub revision: u64,
    pub node_positions: BTreeMap<String, NodePosition>,
    pub identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticSnapshot {
    pub source_semantic_hash: String,
    pub descriptor_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanSnapshot {
    pub identity: String,
    pub source_semantic_hash: String,
}

/// A run is pinned to its resolved plan even if source changes later.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub run_id: String,
    pub plan_identity: String,
    pub source_semantic_hash: String,
    pub state: RunState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunState {
    Prepared,
    Running,
    Terminal,
}

/// Addressing remains explicit about authored/logical versus expanded paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SubjectPath {
    Logical(String),
    Expanded(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCursor {
    pub stream_id: String,
    pub cursor: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProjectionUpdate {
    Snapshot {
        cursor: EvidenceCursor,
    },
    Delta {
        cursor: EvidenceCursor,
        subject: SubjectPath,
    },
    /// Consumers must obtain a new snapshot; they may not infer missing state.
    Gap {
        requested: u64,
        earliest_available: u64,
    },
}

/// A finite reference projection stream. The authoritative evidence stream is
/// owned by Resonance; this only models Patchbay's rebuildable view cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionLog {
    stream_id: String,
    capacity: usize,
    earliest_available: u64,
    next_cursor: u64,
    subjects: Vec<SubjectPath>,
}

impl ProjectionLog {
    pub fn new(stream_id: impl Into<String>, capacity: usize) -> Result<Self, ProtocolError> {
        if capacity == 0 {
            return Err(ProtocolError {
                code: "CND-PBY-006",
                message: "projection retention capacity must be finite and nonzero".to_owned(),
                diagnostics: Vec::new(),
            });
        }
        Ok(Self {
            stream_id: stream_id.into(),
            capacity,
            earliest_available: 1,
            next_cursor: 1,
            subjects: Vec::new(),
        })
    }

    pub fn append(&mut self, subject: SubjectPath) -> EvidenceCursor {
        let cursor = EvidenceCursor {
            stream_id: self.stream_id.clone(),
            cursor: self.next_cursor,
        };
        self.next_cursor += 1;
        self.subjects.push(subject);
        if self.subjects.len() > self.capacity {
            self.subjects.remove(0);
            self.earliest_available += 1;
        }
        cursor
    }

    /// Returns a gap rather than inventing missing projection deltas.
    #[must_use]
    pub fn observe_from(&self, cursor: u64) -> Vec<ProjectionUpdate> {
        if cursor.saturating_add(1) < self.earliest_available {
            return vec![ProjectionUpdate::Gap {
                requested: cursor,
                earliest_available: self.earliest_available,
            }];
        }
        let first = cursor.saturating_add(1).max(self.earliest_available);
        self.subjects
            .iter()
            .enumerate()
            .filter_map(|(index, subject)| {
                let event_cursor = self.earliest_available + index as u64;
                (event_cursor >= first).then(|| ProjectionUpdate::Delta {
                    cursor: EvidenceCursor {
                        stream_id: self.stream_id.clone(),
                        cursor: event_cursor,
                    },
                    subject: subject.clone(),
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EditOperation {
    /// Replaces editable source atomically after it parses through conduit-panel.
    ReplaceSource { source: String },
    /// Updates layout only. The node name is validated against parsed source.
    MoveNode {
        node_id: String,
        position: NodePosition,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EditRequest {
    pub protocol_version: u16,
    pub document_id: String,
    pub expected_source_revision: u64,
    pub expected_presentation_revision: u64,
    pub operations: Vec<EditOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EditResult {
    pub source: SourceSnapshot,
    pub presentation: PresentationSnapshot,
    pub semantic: SemanticSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    pub code: &'static str,
    pub message: String,
    pub diagnostics: Vec<String>,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProtocolError {}

/// In-memory reference model for atomic Patchbay edits. Network transports
/// adapt these versioned request and result values; they do not alter them.
#[derive(Clone, Debug)]
pub struct Workspace {
    source: SourceSnapshot,
    presentation: PresentationSnapshot,
    descriptor_identity: Option<String>,
}

impl Workspace {
    pub fn new(
        document_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, ProtocolError> {
        let document_id = document_id.into();
        let source = source.into();
        let document = conduit_panel::parse_document(&source);
        let semantic_hash = document.semantic_hash_v2().ok_or_else(|| ProtocolError {
            code: "CND-PBY-004",
            message: "initial source must parse".to_owned(),
            diagnostics: document
                .diagnostics
                .iter()
                .map(ToString::to_string)
                .collect(),
        })?;
        let presentation = presentation_snapshot(&document_id, 0, BTreeMap::new());
        Ok(Self {
            source: SourceSnapshot {
                document_id,
                revision: 0,
                source,
                semantic_hash,
            },
            presentation,
            descriptor_identity: None,
        })
    }

    #[must_use]
    pub fn source(&self) -> &SourceSnapshot {
        &self.source
    }

    #[must_use]
    pub fn presentation(&self) -> &PresentationSnapshot {
        &self.presentation
    }

    #[must_use]
    pub fn semantic(&self) -> SemanticSnapshot {
        SemanticSnapshot {
            source_semantic_hash: self.source.semantic_hash.clone(),
            descriptor_identity: self.descriptor_identity.clone(),
        }
    }

    /// Binds an externally resolved descriptor without making it source or
    /// presentation state. Callers must supply the exact resolver result.
    pub fn set_descriptor_identity(&mut self, identity: Option<String>) {
        self.descriptor_identity = identity;
    }

    pub fn apply(&mut self, request: EditRequest) -> Result<EditResult, ProtocolError> {
        if request.protocol_version != PATCHBAY_PROTOCOL_V1 {
            return Err(ProtocolError {
                code: "CND-PBY-001",
                message: "unsupported Patchbay protocol version".to_owned(),
                diagnostics: Vec::new(),
            });
        }
        if request.document_id != self.source.document_id {
            return Err(ProtocolError {
                code: "CND-PBY-002",
                message: "request names another source document".to_owned(),
                diagnostics: Vec::new(),
            });
        }
        if request.expected_source_revision != self.source.revision
            || request.expected_presentation_revision != self.presentation.revision
        {
            return Err(ProtocolError {
                code: "CND-PBY-003",
                message: "stale source or presentation base revision".to_owned(),
                diagnostics: Vec::new(),
            });
        }

        let mut candidate_source = self.source.clone();
        let mut positions = self.presentation.node_positions.clone();
        let mut source_changed = false;
        let mut presentation_changed = false;
        for operation in request.operations {
            match operation {
                EditOperation::ReplaceSource { source } => {
                    let document = conduit_panel::parse_document(&source);
                    let semantic_hash =
                        document.semantic_hash_v2().ok_or_else(|| ProtocolError {
                            code: "CND-PBY-004",
                            message: "source edit did not parse; transaction was not applied"
                                .to_owned(),
                            diagnostics: document
                                .diagnostics
                                .iter()
                                .map(ToString::to_string)
                                .collect(),
                        })?;
                    candidate_source.source = source;
                    candidate_source.semantic_hash = semantic_hash;
                    source_changed = true;
                }
                EditOperation::MoveNode { node_id, position } => {
                    let document = conduit_panel::parse_document(&candidate_source.source);
                    let panel = document.panel().map_err(|error| ProtocolError {
                        code: "CND-PBY-004",
                        message: "current source is not editable".to_owned(),
                        diagnostics: vec![error.to_string()],
                    })?;
                    if !panel.nodes.iter().any(|node| node.id == node_id) {
                        return Err(ProtocolError {
                            code: "CND-PBY-005",
                            message: format!("unknown source node `{node_id}`"),
                            diagnostics: Vec::new(),
                        });
                    }
                    positions.insert(node_id, position);
                    presentation_changed = true;
                }
            }
        }
        if source_changed {
            candidate_source.revision += 1;
        }
        let presentation_revision = self.presentation.revision + u64::from(presentation_changed);
        let candidate_presentation =
            presentation_snapshot(&self.source.document_id, presentation_revision, positions);
        self.source = candidate_source;
        self.presentation = candidate_presentation;
        Ok(EditResult {
            source: self.source.clone(),
            presentation: self.presentation.clone(),
            semantic: self.semantic(),
        })
    }
}

fn presentation_snapshot(
    document_id: &str,
    revision: u64,
    node_positions: BTreeMap<String, NodePosition>,
) -> PresentationSnapshot {
    let mut identity_input =
        format!("conduit.patchbay-presentation/v1\0{document_id}\0{revision}\0");
    for (node, position) in &node_positions {
        identity_input.push_str(&format!("{node}\0{}\0{}\0", position.x, position.y));
    }
    let identity = format!("sha256:{:x}", Sha256::digest(identity_input.as_bytes()));
    PresentationSnapshot {
        document_id: document_id.to_owned(),
        revision,
        node_positions,
        identity,
    }
}
