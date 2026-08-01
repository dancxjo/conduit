//! Transport-neutral Patchbay authoring and observation protocol.
//!
//! This crate intentionally owns only mutable authoring and presentation
//! projections.  It never makes layout part of `.panel` semantics, resolves a
//! plan, executes a node, or appends executor evidence.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const PATCHBAY_PROTOCOL_VERSION: u16 = 0;
pub const DEFAULT_WORKSPACE_HISTORY_LIMIT: usize = 16;
pub const MAXIMUM_EDIT_OPERATIONS: usize = 32;
pub const MAXIMUM_PATCHBAY_DIAGNOSTICS: usize = 64;
pub const MAXIMUM_LIBRARY_CATALOG_ENTRIES: usize = 512;

/// Bounded presentation of the checked library catalog. Known provider
/// bundles remain distinct from current host observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LibraryCatalogProjection {
    pub schema: String,
    pub entries: Vec<LibraryCatalogEntryProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LibraryCatalogEntryProjection {
    pub semantic_identity: String,
    pub public_source_spelling: String,
    pub classification: String,
    pub package_owner: String,
    pub compiler_exported: bool,
    pub known_provider_bundles: Vec<String>,
    pub current_provider_observation: String,
    pub conformance_fixture_owner: String,
    pub standalone_lesson: LibraryLessonProjection,
    pub composition_lesson: LibraryLessonProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LibraryLessonProjection {
    pub artifact: String,
    pub status: String,
}

#[derive(Deserialize)]
struct LibraryCatalogDocument {
    schema: String,
    schema_version: u32,
    entries: Vec<LibraryCatalogDocumentEntry>,
}

#[derive(Deserialize)]
struct LibraryCatalogDocumentEntry {
    semantic_identity: String,
    public_source_spelling: String,
    classification: String,
    package_owner: String,
    compiler_exported: bool,
    known_provider_bundles: Vec<LibraryProviderDocument>,
    current_provider_observation: String,
    conformance_fixture_owner: String,
    standalone_lesson: LibraryLessonProjection,
    composition_lesson: LibraryLessonProjection,
}

#[derive(Deserialize)]
struct LibraryProviderDocument {
    implementation: String,
}

/// Projects checked catalog data without discovering providers, reading host
/// state, loading code, or granting authority.
pub fn project_library_catalog(json: &str) -> Result<LibraryCatalogProjection, ProtocolError> {
    let document: LibraryCatalogDocument = serde_json::from_str(json)
        .map_err(|_| rejected("CND-PBY-014", "invalid library catalog document"))?;
    if document.schema != "conduit.library-catalog"
        || document.schema_version != 0
        || document.entries.len() > MAXIMUM_LIBRARY_CATALOG_ENTRIES
    {
        return Err(rejected(
            "CND-PBY-014",
            "unsupported or oversized library catalog document",
        ));
    }
    let mut identities = std::collections::BTreeSet::new();
    let mut entries = Vec::with_capacity(document.entries.len());
    for entry in document.entries {
        if !identities.insert(entry.semantic_identity.clone())
            || ![
                "portable-standard",
                "optional-host-boundary",
                "reusable-domain-package",
                "implementation-helper",
                "provisional-removal",
            ]
            .contains(&entry.classification.as_str())
            || entry.current_provider_observation != "not-recorded-in-catalog"
        {
            return Err(rejected(
                "CND-PBY-014",
                "library catalog identity, class, or observation boundary is invalid",
            ));
        }
        entries.push(LibraryCatalogEntryProjection {
            semantic_identity: entry.semantic_identity,
            public_source_spelling: entry.public_source_spelling,
            classification: entry.classification,
            package_owner: entry.package_owner,
            compiler_exported: entry.compiler_exported,
            known_provider_bundles: entry
                .known_provider_bundles
                .into_iter()
                .map(|provider| provider.implementation)
                .collect(),
            current_provider_observation: entry.current_provider_observation,
            conformance_fixture_owner: entry.conformance_fixture_owner,
            standalone_lesson: entry.standalone_lesson,
            composition_lesson: entry.composition_lesson,
        });
    }
    Ok(LibraryCatalogProjection {
        schema: document.schema,
        entries,
    })
}

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
    /// Exact identity of the current UTF-8 source, including trivia.
    pub identity: String,
    /// Semantic identity exists only when the current source parses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_hash: Option<String>,
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
pub struct NodeAvailabilityProjection {
    pub contract_id: String,
    pub availability_state: String,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rejection_reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_semantic_hash: Option<String>,
    pub descriptor_identity: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub availabilities: Vec<NodeAvailabilityProjection>,
}

/// Versioned, bounded renderer input. Every field is copied from an
/// authoritative Rust resource; presentation clients may arrange these facts
/// but may not infer missing semantic or runtime state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayViewModel {
    pub protocol_version: u16,
    pub source: SourceSnapshot,
    pub semantic: SemanticSnapshot,
    pub presentation: PresentationSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<RunSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high_water: Option<PatchbayHighWaterProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub evidence: Vec<serde_json::Value>,
    pub topology: PatchbayTopologyProjection,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostics: Vec<PatchbayDiagnosticProjection>,
    pub bounds: PatchbayProjectionBounds,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayHighWaterProjection {
    pub queue_items: u64,
    pub queue_payload_bytes: u64,
    pub ready_slots: u32,
    pub event_slots: u32,
    pub decisions: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayProjectionBounds {
    pub maximum_nodes: usize,
    pub maximum_cords: usize,
    pub maximum_composites: usize,
    pub maximum_ports_per_node: usize,
    pub maximum_config_fields_per_node: usize,
    pub maximum_evidence_events: usize,
    pub maximum_diagnostics: usize,
    pub maximum_history: usize,
}

impl Default for PatchbayProjectionBounds {
    fn default() -> Self {
        Self {
            maximum_nodes: 1_024,
            maximum_cords: 4_096,
            maximum_composites: 1_024,
            maximum_ports_per_node: 256,
            maximum_config_fields_per_node: 256,
            maximum_evidence_events: 256,
            maximum_diagnostics: MAXIMUM_PATCHBAY_DIAGNOSTICS,
            maximum_history: DEFAULT_WORKSPACE_HISTORY_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayTopologyProjection {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contract_imports: Vec<PatchbayContractImportProjection>,
    pub logical_nodes: Vec<PatchbayNodeProjection>,
    pub expanded_nodes: Vec<PatchbayNodeProjection>,
    pub cords: Vec<PatchbayCordProjection>,
    pub composites: Vec<PatchbayCompositeProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_anchors: Vec<PatchbayDiagnosticAnchorProjection>,
    /// `exact`, `invalid`, or `partial` for the current source revision.
    pub source_state: String,
}

/// Checked source alias alongside its immutable semantic identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayContractImportProjection {
    pub local_name: String,
    pub package_id: String,
    pub canonical_id: String,
    pub descriptor_hash: String,
}

/// Projects resolved import facts without adding provider or availability claims.
#[must_use]
pub fn project_contract_imports(
    resolution: &conduit_panel::PackageImportResolution,
) -> Vec<PatchbayContractImportProjection> {
    resolution
        .bindings()
        .iter()
        .map(|binding| PatchbayContractImportProjection {
            local_name: binding.local_name.clone(),
            package_id: binding.package_id.clone(),
            canonical_id: binding.canonical_id.clone(),
            descriptor_hash: binding.descriptor_hash.clone(),
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayNodeProjection {
    pub id: String,
    pub semantic_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeProjection>,
    pub inputs: Vec<PatchbayPortProjection>,
    pub outputs: Vec<PatchbayPortProjection>,
    pub config: BTreeMap<String, PatchbayConfigProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<NodeAvailabilityProjection>,
    pub validity: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayConfigProjection {
    pub kind: String,
    pub display_value: String,
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeProjection>,
    pub validity: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayPortProjection {
    pub id: String,
    /// Stable semantic selection subject; never derived from screen geometry.
    pub semantic_path: String,
    pub direction: String,
    /// Exact full face/list spelling with the one-glyph flow convention.
    pub display_label: String,
    /// Redundant non-color description for assistive presentation.
    pub accessible_label: String,
    pub type_id: String,
    pub delivery: String,
    pub connections: String,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeProjection>,
    pub validity: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayCordProjection {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port_range: Option<SourceRangeProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port_range: Option<SourceRangeProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityProof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_items: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_queued_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low_watermark_items: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high_watermark_items: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high_water_items: Option<u16>,
    pub validity: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_from_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_from_port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_to_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_to_port: Option<String>,
}

/// A presentation endpoint for authored syntax which does not resolve to a
/// semantic/runtime port. It is deliberately separate from node ports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayDiagnosticAnchorProjection {
    pub id: String,
    pub cord_id: String,
    pub side: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayDiagnosticTargetProjection {
    pub kind: String,
    pub id: String,
}

/// One Rust-authored, element-scoped diagnostic for the current source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayDiagnosticProjection {
    pub id: String,
    pub code: String,
    pub severity: String,
    pub state: String,
    pub message: String,
    pub explanation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_range: Option<SourceRangeProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub related_ranges: Vec<SourceRangeProjection>,
    pub targets: Vec<PatchbayDiagnosticTargetProjection>,
}

/// Exact authored range for one projected topology item.
///
/// Byte offsets preserve Rust source identity. UTF-16 offsets are supplied
/// separately for browser textarea APIs, so the presentation layer never
/// guesses across non-ASCII source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRangeProjection {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub source_revision: u64,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayCompositeProjection {
    pub id: String,
    pub definition: String,
    pub members: Vec<String>,
    pub exports: Vec<PatchbayExportProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayExportProjection {
    pub direction: String,
    pub id: String,
    pub target_node: String,
    pub target_port: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityProof {
    pub compatible: bool,
    pub code: String,
    pub producer_type: Option<String>,
    pub consumer_type: Option<String>,
    pub candidate_plan_identity: Option<String>,
    /// `candidate-only` means source may commit but no active plan changes;
    /// activation requires the bounded #57 transition protocol.
    pub plan_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanBindingProjection {
    pub instance: String,
    pub contract_id: String,
    pub contract_identity: String,
    pub implementation_id: String,
    pub implementation_identity: String,
    pub host_id: String,
    pub host_observation_id: String,
    pub host_observation_identity: String,
    pub availability_state: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanSnapshot {
    pub identity: String,
    pub source_semantic_hash: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bindings: Vec<PlanBindingProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub value_envelopes: Vec<ValueEnvelopeProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub watch_admissions: Vec<WatchAdmissionProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub clock_conversions: Vec<ClockConversionProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub feedback_boundaries: Vec<FeedbackBoundaryProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub resource_leases: Vec<ResourceLeaseProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub effect_commit_profiles: Vec<EffectCommitProjection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub workloads: Vec<WorkloadProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WatchAdmissionProjection {
    pub id: String,
    pub subject_kind: String,
    pub cord: Option<String>,
    pub node: Option<String>,
    pub port: Option<String>,
    pub direction: Option<String>,
    pub representation_id: String,
    pub representation_identity: String,
    pub maximum_preview_bytes: u32,
    pub maximum_history: u16,
    pub minimum_tick_interval: u64,
    pub retention: String,
    pub sensitivity_ceiling: String,
    pub reveal_action: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudgetProjection {
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub cpu_units: u32,
    pub timers: u16,
    pub transports: u16,
    pub checkpoints: u16,
    pub evidence_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceLeaseProjection {
    pub id: String,
    pub resource_binding: String,
    pub holder: String,
    pub run: String,
    pub epoch: u64,
    pub scope: String,
    pub sharing: String,
    pub maximum_holders: u16,
    pub reservation: ResourceBudgetProjection,
    pub time_basis: String,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub revocation_grace_ticks: u64,
    pub cleanup_ticks: u64,
    pub maximum_operations: u32,
    pub maximum_evidence_events: u32,
    pub cleanup_escalation_id: String,
    pub cleanup_escalation_identity: String,
    pub foreign_retention: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectCommitProjection {
    pub node: String,
    pub id: String,
    pub operation: String,
    pub resource_lease: String,
    pub commit_boundary_id: String,
    pub commit_boundary_identity: String,
    pub idempotency: String,
    pub unknown_commit: String,
    pub discontinuity: String,
    pub cleanup_id: String,
    pub cleanup_identity: String,
    pub maximum_attempts: u16,
    pub evidence_events_per_attempt: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadBudgetProjection {
    pub work_units: Option<u64>,
    pub tasks: Option<u64>,
    pub processes: Option<u64>,
    pub descriptors: Option<u64>,
    pub connections: Option<u64>,
    pub storage_bytes: Option<u64>,
    pub device_operations: Option<u64>,
    pub network_bytes: Option<u64>,
    pub callbacks: Option<u64>,
    pub foreign_queue_items: Option<u64>,
    pub transition_overlap_work_units: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadProjection {
    pub id: String,
    pub service: String,
    pub node: String,
    pub guarantee: String,
    pub budget: WorkloadBudgetProjection,
    pub deadline_time_basis: Option<String>,
    pub relative_deadline_ticks: Option<u64>,
    pub maximum_jitter_ticks: Option<u64>,
    pub capability_id: String,
    pub capability_identity: String,
    pub host_observation: String,
    pub evidence_kind: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub maximum_evidence_events: u32,
}

/// Presentation-only cross-host chain. These categories remain separate so
/// Patchbay cannot present a descriptor as installed, available, conformant,
/// or exactly bound merely because it was discovered.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostConformanceProjection {
    pub profile_id: String,
    pub profile_identity: String,
    pub host_class: String,
    pub execution_mode: String,
    pub mandatory_facts: Vec<IdentityProjection>,
    pub optional_providers: Vec<OptionalProviderProjection>,
    pub extensions: Vec<HostExtensionProjection>,
    pub exact_bindings: Vec<ExactProviderBindingProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityProjection {
    pub id: String,
    pub identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OptionalProviderProjection {
    pub contract: IdentityProjection,
    pub provider_bundle: IdentityProjection,
    pub inventory_state: String,
    pub observation_state: Option<String>,
    pub host_observation: Option<IdentityProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostExtensionProjection {
    pub kind: String,
    pub descriptor: IdentityProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExactProviderBindingProjection {
    pub required_contract: IdentityProjection,
    pub offered_facets: Vec<IdentityProjection>,
    pub satisfaction_proof: String,
    pub conformance_result: String,
    pub provider_bundle: IdentityProjection,
    pub host_observation: IdentityProjection,
    pub implementation: IdentityProjection,
    pub artifact: IdentityProjection,
    pub adapter: IdentityProjection,
    pub maximum_in_flight: u16,
    pub maximum_foreign_queue: u16,
    pub maximum_memory_bytes: u64,
    pub maximum_cancellation_ticks: u64,
    pub maximum_evidence_events: u32,
}

pub struct HostConformanceProjectionInput<'a> {
    pub profile_pin: conduit_core::PinnedDescriptor<'a>,
    pub profile: conduit_core::HostConformanceProfile<'a>,
    pub observations: &'a [conduit_core::ProviderObservation<'a>],
    pub conformance_results: &'a [conduit_core::ProviderConformanceResult<'a>],
    pub bindings: &'a [conduit_core::ExactProviderBinding<'a>],
}

#[must_use]
pub fn project_host_conformance(
    input: HostConformanceProjectionInput<'_>,
) -> HostConformanceProjection {
    let optional_providers = input
        .profile
        .optional_providers
        .iter()
        .map(|provider| {
            let observation = input
                .observations
                .iter()
                .find(|observation| observation.provider_bundle == provider.provider_bundle);
            OptionalProviderProjection {
                contract: identity_projection(provider.contract),
                provider_bundle: identity_projection(provider.provider_bundle),
                inventory_state: provider.state.as_str().to_owned(),
                observation_state: observation.map(|value| value.state.as_str().to_owned()),
                host_observation: observation.map(|value| identity_projection(value.host_report)),
            }
        })
        .collect();
    let exact_bindings = input
        .bindings
        .iter()
        .map(|binding| {
            let conformance = input
                .conformance_results
                .iter()
                .find(|result| result.identity == binding.conformance_result);
            ExactProviderBindingProjection {
                required_contract: identity_projection(binding.required_contract),
                offered_facets: conformance
                    .map(|result| {
                        result
                            .offered_facets
                            .iter()
                            .copied()
                            .map(identity_projection)
                            .collect()
                    })
                    .unwrap_or_default(),
                satisfaction_proof: binding.satisfaction_proof.to_string(),
                conformance_result: binding.conformance_result.to_string(),
                provider_bundle: identity_projection(binding.provider_bundle),
                host_observation: identity_projection(binding.host_report),
                implementation: identity_projection(binding.implementation),
                artifact: identity_projection(binding.artifact),
                adapter: identity_projection(binding.adapter),
                maximum_in_flight: binding.bounds.maximum_in_flight,
                maximum_foreign_queue: binding.bounds.maximum_foreign_queue,
                maximum_memory_bytes: binding.bounds.maximum_memory_bytes,
                maximum_cancellation_ticks: binding.bounds.maximum_cancellation_ticks,
                maximum_evidence_events: binding.bounds.maximum_evidence_events,
            }
        })
        .collect();
    HostConformanceProjection {
        profile_id: input.profile.id.as_str().to_owned(),
        profile_identity: input.profile_pin.semantic_hash.to_string(),
        host_class: input.profile.class.as_str().to_owned(),
        execution_mode: input.profile.execution_mode.as_str().to_owned(),
        mandatory_facts: input
            .profile
            .mandatory_facts
            .iter()
            .copied()
            .map(identity_projection)
            .collect(),
        optional_providers,
        extensions: input
            .profile
            .extensions
            .iter()
            .map(|extension| HostExtensionProjection {
                kind: extension.kind.as_str().to_owned(),
                descriptor: identity_projection(extension.descriptor),
            })
            .collect(),
        exact_bindings,
    }
}

fn identity_projection(value: conduit_core::PinnedDescriptor<'_>) -> IdentityProjection {
    IdentityProjection {
        id: value.id.as_str().to_owned(),
        identity: value.semantic_hash.to_string(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValueEnvelopeProjection {
    pub cord: String,
    pub representation_id: String,
    pub representation_identity: String,
    pub maximum_payload_bytes: u32,
    pub maximum_envelope_bytes: u32,
    pub maximum_fragments: u16,
    pub maximum_fragment_bytes: u32,
    pub maximum_timestamps: u8,
    pub clock_domains: Vec<String>,
    pub identity_allowed: bool,
    pub correlation_allowed: bool,
    pub causation_allowed: bool,
    pub provenance_allowed: bool,
    pub sensitivity_ceiling: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClockConversionProjection {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub numerator: u64,
    pub denominator: u64,
    pub offset_ticks: i64,
    pub rounding: String,
    pub maximum_uncertainty_ticks: u64,
    pub observed_time_basis: String,
    pub observed_tick: u64,
    pub valid_until_tick: u64,
    pub authority: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeedbackBoundaryProjection {
    pub id: String,
    pub node: String,
    pub cord: String,
    pub kind: String,
    pub initialization: String,
    pub initial_items: u16,
    pub initial_bytes: u64,
    pub maximum_retained_items: u16,
    pub maximum_retained_bytes: u64,
    pub delay_ticks: u64,
    pub clock: Option<String>,
    pub replay_gap: String,
    pub cancellation_id: String,
    pub cancellation_identity: String,
    pub terminal: String,
}

impl PlanSnapshot {
    #[must_use]
    pub fn from_exact_plan(plan: &conduit_core::ExecutionPlan<'_>) -> Self {
        let bindings = plan
            .nodes
            .iter()
            .map(|node| {
                let observation = plan
                    .host_observations
                    .iter()
                    .find(|observation| observation.id == node.host_observation)
                    .expect("validated exact plan nodes name an existing host observation");
                PlanBindingProjection {
                    instance: node.instance.as_str().to_owned(),
                    contract_id: node.contract.id.as_str().to_owned(),
                    contract_identity: node.contract.semantic_hash.to_string(),
                    implementation_id: node.implementation.id.as_str().to_owned(),
                    implementation_identity: node.implementation.semantic_hash.to_string(),
                    host_id: node.host.as_str().to_owned(),
                    host_observation_id: observation.id.as_str().to_owned(),
                    host_observation_identity: observation.semantic_hash.to_string(),
                    availability_state: "bound-in-this-plan".to_owned(),
                    reason_code: "CND-AVL-004".to_owned(),
                }
            })
            .collect();
        let value_envelopes = plan
            .value_envelopes
            .iter()
            .map(|policy| ValueEnvelopeProjection {
                cord: policy.cord.as_str().to_owned(),
                representation_id: policy.representation.id.as_str().to_owned(),
                representation_identity: policy.representation.semantic_hash.to_string(),
                maximum_payload_bytes: policy.maximum_payload_bytes,
                maximum_envelope_bytes: policy.maximum_envelope_bytes,
                maximum_fragments: policy.maximum_fragments,
                maximum_fragment_bytes: policy.maximum_fragment_bytes,
                maximum_timestamps: policy.maximum_timestamps,
                clock_domains: policy
                    .clock_domains
                    .iter()
                    .map(|clock| clock.as_str().to_owned())
                    .collect(),
                identity_allowed: policy.identity_allowed,
                correlation_allowed: policy.correlation_allowed,
                causation_allowed: policy.causation_allowed,
                provenance_allowed: policy.provenance_allowed,
                sensitivity_ceiling: match policy.sensitivity_ceiling {
                    conduit_core::Sensitivity::Public => "public",
                    conduit_core::Sensitivity::Restricted => "restricted",
                    conduit_core::Sensitivity::Secret => "secret",
                }
                .to_owned(),
            })
            .collect();
        let watch_admissions = plan
            .watch_admissions
            .iter()
            .map(|watch| {
                let (subject_kind, cord, node, port, direction) = match watch.subject {
                    conduit_core::WatchSubject::Cord(cord) => (
                        "cord".to_owned(),
                        Some(cord.as_str().to_owned()),
                        None,
                        None,
                        None,
                    ),
                    conduit_core::WatchSubject::NodePort {
                        node,
                        port,
                        direction,
                    } => (
                        "node-port".to_owned(),
                        None,
                        Some(node.as_str().to_owned()),
                        Some(port.as_str().to_owned()),
                        Some(direction.as_str().to_owned()),
                    ),
                };
                WatchAdmissionProjection {
                    id: watch.id.as_str().to_owned(),
                    subject_kind,
                    cord,
                    node,
                    port,
                    direction,
                    representation_id: watch.representation.id.as_str().to_owned(),
                    representation_identity: watch.representation.semantic_hash.to_string(),
                    maximum_preview_bytes: watch.maximum_preview_bytes,
                    maximum_history: watch.maximum_history,
                    minimum_tick_interval: watch.minimum_tick_interval,
                    retention: watch.retention.as_str().to_owned(),
                    sensitivity_ceiling: watch.sensitivity_ceiling.as_str().to_owned(),
                    reveal_action: watch.reveal_action.map(|action| action.as_str().to_owned()),
                }
            })
            .collect();
        let clock_conversions = plan
            .clock_conversions
            .iter()
            .map(|conversion| ClockConversionProjection {
                id: conversion.id.as_str().to_owned(),
                source: conversion.source.as_str().to_owned(),
                destination: conversion.destination.as_str().to_owned(),
                numerator: conversion.numerator,
                denominator: conversion.denominator,
                offset_ticks: conversion.offset_ticks,
                rounding: match conversion.rounding {
                    conduit_core::ClockRounding::Exact => "exact",
                    conduit_core::ClockRounding::Floor => "floor",
                    conduit_core::ClockRounding::Ceiling => "ceiling",
                }
                .to_owned(),
                maximum_uncertainty_ticks: conversion.maximum_uncertainty_ticks,
                observed_time_basis: conversion.observed_at.basis.as_str().to_owned(),
                observed_tick: conversion.observed_at.tick,
                valid_until_tick: conversion.valid_until_tick,
                authority: conversion.authority.as_str().to_owned(),
            })
            .collect();
        let feedback_boundaries = plan
            .feedback_boundaries
            .iter()
            .map(|boundary| FeedbackBoundaryProjection {
                id: boundary.id.as_str().to_owned(),
                node: boundary.node.as_str().to_owned(),
                cord: boundary.cord.as_str().to_owned(),
                kind: match boundary.kind {
                    conduit_core::FeedbackBoundaryKind::Delay => "delay",
                    conduit_core::FeedbackBoundaryKind::State => "state",
                }
                .to_owned(),
                initialization: match boundary.initialization {
                    conduit_core::FeedbackInitialization::Empty => "empty",
                    conduit_core::FeedbackInitialization::InitialValue => "initial-value",
                }
                .to_owned(),
                initial_items: boundary.initial_items,
                initial_bytes: boundary.initial_bytes,
                maximum_retained_items: boundary.maximum_retained_items,
                maximum_retained_bytes: boundary.maximum_retained_bytes,
                delay_ticks: boundary.delay_ticks,
                clock: boundary.clock.map(|clock| clock.as_str().to_owned()),
                replay_gap: match boundary.replay_gap {
                    conduit_core::FeedbackReplayGapPolicy::Fail => "fail",
                    conduit_core::FeedbackReplayGapPolicy::Reset => "reset",
                    conduit_core::FeedbackReplayGapPolicy::Wait => "wait",
                }
                .to_owned(),
                cancellation_id: boundary.cancellation.id.as_str().to_owned(),
                cancellation_identity: boundary.cancellation.semantic_hash.to_string(),
                terminal: match boundary.terminal {
                    conduit_core::FeedbackTerminalPolicy::DropRetained => "drop-retained",
                    conduit_core::FeedbackTerminalPolicy::DrainRetained => "drain-retained",
                }
                .to_owned(),
            })
            .collect();
        let resource_leases = plan
            .resources
            .iter()
            .filter_map(|resource| resource.lease)
            .map(|lease| ResourceLeaseProjection {
                id: lease.id.as_str().to_owned(),
                resource_binding: lease.resource_binding.as_str().to_owned(),
                holder: lease.holder.as_str().to_owned(),
                run: lease.run.as_str().to_owned(),
                epoch: lease.epoch,
                scope: lease.scope.as_str().to_owned(),
                sharing: match lease.sharing {
                    conduit_core::ResourceSharingMode::Exclusive => "exclusive",
                    conduit_core::ResourceSharingMode::SharedRead => "shared-read",
                    conduit_core::ResourceSharingMode::SharedBounded { .. } => "shared-bounded",
                }
                .to_owned(),
                maximum_holders: match lease.sharing {
                    conduit_core::ResourceSharingMode::Exclusive => 1,
                    conduit_core::ResourceSharingMode::SharedRead => u16::MAX,
                    conduit_core::ResourceSharingMode::SharedBounded { maximum_holders } => {
                        maximum_holders
                    }
                },
                reservation: ResourceBudgetProjection {
                    memory_bytes: lease.reservation.memory_bytes,
                    storage_bytes: lease.reservation.storage_bytes,
                    cpu_units: lease.reservation.cpu_units,
                    timers: lease.reservation.timers,
                    transports: lease.reservation.transports,
                    checkpoints: lease.reservation.checkpoints,
                    evidence_bytes: lease.reservation.evidence_bytes,
                },
                time_basis: lease.time_basis.as_str().to_owned(),
                issued_at_tick: lease.issued_at_tick,
                expires_at_tick: lease.expires_at_tick,
                revocation_grace_ticks: lease.revocation_grace_ticks,
                cleanup_ticks: lease.cleanup_ticks,
                maximum_operations: lease.maximum_operations,
                maximum_evidence_events: lease.maximum_evidence_events,
                cleanup_escalation_id: lease.cleanup_escalation.id.as_str().to_owned(),
                cleanup_escalation_identity: lease.cleanup_escalation.semantic_hash.to_string(),
                foreign_retention: match lease.foreign_retention {
                    conduit_core::ForeignRetention::None => "none",
                    conduit_core::ForeignRetention::Bounded { .. } => "bounded",
                    conduit_core::ForeignRetention::ObservedOnly => "observed-only",
                    conduit_core::ForeignRetention::Unsupported => "unsupported",
                }
                .to_owned(),
            })
            .collect();
        let effect_commit_profiles = plan
            .authorities
            .iter()
            .filter_map(|authority| {
                authority
                    .commit_profile
                    .map(|profile| (authority.node, profile))
            })
            .map(|(node, profile)| EffectCommitProjection {
                node: node.as_str().to_owned(),
                id: profile.id.as_str().to_owned(),
                operation: profile.operation.as_str().to_owned(),
                resource_lease: profile.resource_lease.as_str().to_owned(),
                commit_boundary_id: profile.commit_boundary.id.as_str().to_owned(),
                commit_boundary_identity: profile.commit_boundary.semantic_hash.to_string(),
                idempotency: match profile.idempotency {
                    conduit_core::EffectIdempotency::None => "none",
                    conduit_core::EffectIdempotency::SameKeySameEffect => "same-key-same-effect",
                    conduit_core::EffectIdempotency::ReconcileBeforeRetry => {
                        "reconcile-before-retry"
                    }
                }
                .to_owned(),
                unknown_commit: match profile.unknown_commit {
                    conduit_core::UnknownCommitPolicy::Fail => "fail",
                    conduit_core::UnknownCommitPolicy::Reconcile => "reconcile",
                    conduit_core::UnknownCommitPolicy::RetrySameIdempotencyKey => {
                        "retry-same-idempotency-key"
                    }
                }
                .to_owned(),
                discontinuity: match profile.discontinuity {
                    conduit_core::EffectDiscontinuity::FailedBeforeCommit => "failed-before-commit",
                    conduit_core::EffectDiscontinuity::CommitUnknown => "commit-unknown",
                    conduit_core::EffectDiscontinuity::ReconcileRequired => "reconcile-required",
                }
                .to_owned(),
                cleanup_id: profile.cleanup.id.as_str().to_owned(),
                cleanup_identity: profile.cleanup.semantic_hash.to_string(),
                maximum_attempts: profile.maximum_attempts,
                evidence_events_per_attempt: profile.evidence_events_per_attempt,
            })
            .collect();
        let workloads = plan
            .workloads
            .iter()
            .map(|workload| WorkloadProjection {
                id: workload.contract.id.as_str().to_owned(),
                service: workload.contract.service.as_str().to_owned(),
                node: workload.contract.node.as_str().to_owned(),
                guarantee: workload.contract.guarantee.as_str().to_owned(),
                budget: workload_budget_projection(workload.contract.budget),
                deadline_time_basis: workload
                    .contract
                    .deadline
                    .map(|deadline| deadline.time_basis.as_str().to_owned()),
                relative_deadline_ticks: workload
                    .contract
                    .deadline
                    .map(|deadline| deadline.relative_deadline_ticks),
                maximum_jitter_ticks: workload
                    .contract
                    .deadline
                    .map(|deadline| deadline.maximum_jitter_ticks),
                capability_id: workload.capability.id.as_str().to_owned(),
                capability_identity: workload.capability.identity.to_string(),
                host_observation: workload.capability.host_observation.as_str().to_owned(),
                evidence_kind: workload.capability.evidence_kind.as_str().to_owned(),
                observed_at_tick: workload.capability.observed_at_tick,
                valid_until_tick: workload.capability.valid_until_tick,
                maximum_evidence_events: workload.contract.maximum_evidence_events,
            })
            .collect();
        Self {
            identity: plan.identity.to_string(),
            source_semantic_hash: plan.source_semantic_hash.to_string(),
            bindings,
            value_envelopes,
            watch_admissions,
            clock_conversions,
            feedback_boundaries,
            resource_leases,
            effect_commit_profiles,
            workloads,
        }
    }
}

fn workload_limit_projection(value: conduit_core::WorkloadLimit) -> Option<u64> {
    match value {
        conduit_core::WorkloadLimit::Finite(value) => Some(value),
        conduit_core::WorkloadLimit::Unsupported => None,
    }
}

fn workload_budget_projection(value: conduit_core::WorkloadBudget) -> WorkloadBudgetProjection {
    WorkloadBudgetProjection {
        work_units: workload_limit_projection(value.work_units),
        tasks: workload_limit_projection(value.tasks),
        processes: workload_limit_projection(value.processes),
        descriptors: workload_limit_projection(value.descriptors),
        connections: workload_limit_projection(value.connections),
        storage_bytes: workload_limit_projection(value.storage_bytes),
        device_operations: workload_limit_projection(value.device_operations),
        network_bytes: workload_limit_projection(value.network_bytes),
        callbacks: workload_limit_projection(value.callbacks),
        foreign_queue_items: workload_limit_projection(value.foreign_queue_items),
        transition_overlap_work_units: workload_limit_projection(
            value.transition_overlap_work_units,
        ),
    }
}

/// A run is pinned to its resolved plan even if source changes later.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub run_id: String,
    pub plan_identity: String,
    pub source_semantic_hash: String,
    pub state: RunState,
}

impl RunSnapshot {
    pub fn from_execution_events(
        plan: &conduit_core::ExecutionPlan<'_>,
        run_id: &str,
        events: &[conduit_core::ExecutionEvent<'_>],
    ) -> Result<Self, ProtocolError> {
        let mut terminal = false;
        for event in events {
            if event.plan_identity != plan.identity || event.run_id.as_str() != run_id {
                return Err(ProtocolError {
                    code: "CND-PBY-008",
                    message: "execution evidence does not match the projected run and exact plan"
                        .to_owned(),
                    diagnostics: Vec::new(),
                    disposition: EditDisposition::Rejected,
                });
            }
            terminal |= matches!(
                event.terminality,
                conduit_core::EventTerminality::Terminal { .. }
            );
        }
        let state = if terminal {
            RunState::Terminal
        } else if events.is_empty() {
            RunState::Prepared
        } else {
            RunState::Active
        };
        Ok(Self {
            run_id: run_id.to_owned(),
            plan_identity: plan.identity.to_string(),
            source_semantic_hash: plan.source_semantic_hash.to_string(),
            state,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunState {
    Prepared,
    /// The exact session has runnable work.
    Active,
    /// The exact session is retained but has no work until an admitted wake.
    Waiting,
    /// The session is draining work admitted before its stop boundary.
    Quiescing,
    /// The session is aborting and bounded provider cleanup is still pending.
    Aborting,
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
                disposition: EditDisposition::Rejected,
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
    /// Adds one explicitly bounded cord. Rust constructs and validates the
    /// candidate source; the browser never appends `.panel` text.
    Connect {
        from_node: String,
        from_port: String,
        to_node: String,
        to_port: String,
        bounds: CordEditBounds,
    },
    /// Removes one parsed cord by its stable source identity.
    Disconnect { cord_id: String },
    /// Replaces one existing typed configuration value at its parser span.
    SetConfig {
        node_id: String,
        key: String,
        value: EditValue,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CordEditBounds {
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum EditValue {
    Boolean(bool),
    Integer(i128),
    Text(String),
    Reference(String),
    ContractReference(String),
    ExactDecimal(String),
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
    pub candidate_revision: CandidateRevision,
    pub diagnostics: Vec<String>,
    pub compatibility: CompatibilityProof,
    pub disposition: EditDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidateRevision {
    pub source: u64,
    pub presentation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EditDisposition {
    Committed,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: &'static str,
    pub message: String,
    pub diagnostics: Vec<String>,
    pub disposition: EditDisposition,
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
    history_limit: usize,
    history: VecDeque<WorkspaceRevision>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRevision {
    pub source: SourceSnapshot,
    pub presentation: PresentationSnapshot,
}

impl Workspace {
    pub fn new(
        document_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, ProtocolError> {
        Self::new_with_history(document_id, source, DEFAULT_WORKSPACE_HISTORY_LIMIT)
    }

    pub fn new_with_history(
        document_id: impl Into<String>,
        source: impl Into<String>,
        history_limit: usize,
    ) -> Result<Self, ProtocolError> {
        if history_limit == 0 {
            return Err(rejected(
                "CND-PBY-006",
                "workspace history capacity must be finite and nonzero",
            ));
        }
        let document_id = document_id.into();
        let source = source.into();
        let document = conduit_panel::parse_document(&source);
        let semantic_hash = document.semantic_hash();
        let presentation = presentation_snapshot(&document_id, 0, BTreeMap::new());
        let source = SourceSnapshot {
            document_id,
            revision: 0,
            identity: exact_source_identity(&source),
            source,
            semantic_hash,
        };
        let mut history = VecDeque::with_capacity(history_limit);
        history.push_back(WorkspaceRevision {
            source: source.clone(),
            presentation: presentation.clone(),
        });
        Ok(Self {
            source,
            presentation,
            descriptor_identity: None,
            history_limit,
            history,
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
    pub fn history(&self) -> &VecDeque<WorkspaceRevision> {
        &self.history
    }

    #[must_use]
    pub fn semantic(&self) -> SemanticSnapshot {
        self.semantic_with_lookup(Self::unsupported_lookup)
    }

    #[must_use]
    pub fn semantic_with_lookup<F>(&self, lookup: F) -> SemanticSnapshot
    where
        F: Fn(&str) -> NodeAvailabilityProjection,
    {
        let mut availabilities = Vec::new();
        let document = conduit_panel::parse_document(&self.source.source);
        if let Ok(panel) = document.panel() {
            for node in &panel.nodes {
                availabilities.push(lookup(node.kind.as_str()));
            }
        }
        SemanticSnapshot {
            source_semantic_hash: self.source.semantic_hash.clone(),
            descriptor_identity: self.descriptor_identity.clone(),
            availabilities,
        }
    }

    /// Binds an externally resolved descriptor without making it source or
    /// presentation state. Callers must supply the exact resolver result.
    pub fn set_descriptor_identity(&mut self, identity: Option<String>) {
        self.descriptor_identity = identity;
    }

    pub fn apply(&mut self, request: EditRequest) -> Result<EditResult, ProtocolError> {
        self.apply_validated(request, Self::unsupported_lookup, |_| {
            Ok(CompatibilityProof {
                compatible: true,
                code: "CND-PBY-VALIDATED".to_owned(),
                producer_type: None,
                consumer_type: None,
                candidate_plan_identity: None,
                plan_disposition: "not-applicable".to_owned(),
            })
        })
    }

    fn unsupported_lookup(kind: &str) -> NodeAvailabilityProjection {
        NodeAvailabilityProjection {
            contract_id: kind.to_owned(),
            availability_state: "unsupported".to_owned(),
            reason_code: "CND-AVL-006".to_owned(),
            implementation_id: None,
            host_id: None,
            rejection_reasons: vec![],
        }
    }

    pub fn apply_with_lookup<F>(
        &mut self,
        request: EditRequest,
        lookup: F,
    ) -> Result<EditResult, ProtocolError>
    where
        F: Fn(&str) -> NodeAvailabilityProjection,
    {
        self.apply_validated(request, lookup, |_| {
            Ok(CompatibilityProof {
                compatible: true,
                code: "CND-PBY-VALIDATED".to_owned(),
                producer_type: None,
                consumer_type: None,
                candidate_plan_identity: None,
                plan_disposition: "not-applicable".to_owned(),
            })
        })
    }

    /// Applies a candidate only after the caller's authoritative
    /// parser/resolver/planner validation succeeds.
    pub fn apply_validated<F, V>(
        &mut self,
        request: EditRequest,
        lookup: F,
        validate: V,
    ) -> Result<EditResult, ProtocolError>
    where
        F: Fn(&str) -> NodeAvailabilityProjection,
        V: Fn(&str) -> Result<CompatibilityProof, ProtocolError>,
    {
        if request.protocol_version != PATCHBAY_PROTOCOL_VERSION {
            return Err(ProtocolError {
                code: "CND-PBY-001",
                message: "unsupported Patchbay protocol version".to_owned(),
                diagnostics: Vec::new(),
                disposition: EditDisposition::Rejected,
            });
        }
        if request.document_id != self.source.document_id {
            return Err(ProtocolError {
                code: "CND-PBY-002",
                message: "request names another source document".to_owned(),
                diagnostics: Vec::new(),
                disposition: EditDisposition::Rejected,
            });
        }
        if request.expected_source_revision != self.source.revision
            || request.expected_presentation_revision != self.presentation.revision
        {
            return Err(ProtocolError {
                code: "CND-PBY-003",
                message: "stale source or presentation base revision".to_owned(),
                diagnostics: Vec::new(),
                disposition: EditDisposition::Rejected,
            });
        }
        if request.operations.len() > MAXIMUM_EDIT_OPERATIONS {
            return Err(rejected(
                "CND-PBY-006",
                "candidate transaction exceeds its finite operation budget",
            ));
        }

        let mut candidate_source = self.source.clone();
        let mut positions = self.presentation.node_positions.clone();
        let mut source_changed = false;
        let mut source_replaced = false;
        let mut presentation_changed = false;
        for operation in request.operations {
            match operation {
                EditOperation::ReplaceSource { source } => {
                    let document = conduit_panel::parse_document(&source);
                    let semantic_hash = document.semantic_hash();
                    candidate_source.identity = exact_source_identity(&source);
                    candidate_source.source = source;
                    candidate_source.semantic_hash = semantic_hash;
                    source_changed = true;
                    source_replaced = true;
                }
                EditOperation::MoveNode { node_id, position } => {
                    let document = conduit_panel::parse_document(&candidate_source.source);
                    let panel = document.panel().map_err(|error| ProtocolError {
                        code: "CND-PBY-004",
                        message: "current source is not editable".to_owned(),
                        diagnostics: vec![error.to_string()],
                        disposition: EditDisposition::Rejected,
                    })?;
                    if !panel.nodes.iter().any(|node| node.id == node_id) {
                        return Err(ProtocolError {
                            code: "CND-PBY-005",
                            message: format!("unknown source node `{node_id}`"),
                            diagnostics: Vec::new(),
                            disposition: EditDisposition::Rejected,
                        });
                    }
                    positions.insert(node_id, position);
                    presentation_changed = true;
                }
                EditOperation::Connect {
                    from_node,
                    from_port,
                    to_node,
                    to_port,
                    bounds,
                } => {
                    validate_cord_bounds(&bounds)?;
                    let panel =
                        conduit_panel::parse(&candidate_source.source).map_err(|error| {
                            rejected_with_diagnostics(
                                "CND-PBY-004",
                                "current source is not editable",
                                vec![error.to_string()],
                            )
                        })?;
                    if !panel.nodes.iter().any(|node| node.id == from_node)
                        || !panel.nodes.iter().any(|node| node.id == to_node)
                    {
                        return Err(rejected(
                            "CND-PBY-005",
                            "connection names an unknown or hidden source node",
                        ));
                    }
                    candidate_source.source.push_str(&canonical_cord_source(
                        &from_node, &from_port, &to_node, &to_port, &bounds,
                    ));
                    source_changed = true;
                }
                EditOperation::Disconnect { cord_id } => {
                    let panel =
                        conduit_panel::parse(&candidate_source.source).map_err(|error| {
                            rejected_with_diagnostics(
                                "CND-PBY-004",
                                "current source is not editable",
                                vec![error.to_string()],
                            )
                        })?;
                    let cord = panel
                        .cords
                        .iter()
                        .find(|cord| cord.id == cord_id)
                        .ok_or_else(|| {
                            rejected("CND-PBY-005", "disconnect names an unknown source cord")
                        })?;
                    remove_source_span(&mut candidate_source.source, cord.source_span)?;
                    source_changed = true;
                }
                EditOperation::SetConfig {
                    node_id,
                    key,
                    value,
                } => {
                    let panel =
                        conduit_panel::parse(&candidate_source.source).map_err(|error| {
                            rejected_with_diagnostics(
                                "CND-PBY-004",
                                "current source is not editable",
                                vec![error.to_string()],
                            )
                        })?;
                    let entry = panel
                        .nodes
                        .iter()
                        .find(|node| node.id == node_id)
                        .and_then(|node| node.config.iter().find(|entry| entry.key == key))
                        .ok_or_else(|| {
                            rejected(
                                "CND-PBY-012",
                                "configuration edit names no existing typed value span",
                            )
                        })?;
                    replace_source_span(
                        &mut candidate_source.source,
                        entry.source_span,
                        &canonical_edit_value(&value),
                    )?;
                    source_changed = true;
                }
            }
        }
        let mut diagnostics = Vec::new();
        let compatibility = if source_changed {
            let document = conduit_panel::parse_document(&candidate_source.source);
            candidate_source.identity = exact_source_identity(&candidate_source.source);
            candidate_source.semantic_hash = document.semantic_hash();
            if source_replaced {
                if let Some(error) = document.diagnostics.first() {
                    diagnostics = document
                        .diagnostics
                        .iter()
                        .take(MAXIMUM_PATCHBAY_DIAGNOSTICS)
                        .map(ToString::to_string)
                        .collect();
                    CompatibilityProof {
                        compatible: false,
                        code: error.code.to_owned(),
                        producer_type: None,
                        consumer_type: None,
                        candidate_plan_identity: None,
                        plan_disposition: "unavailable".to_owned(),
                    }
                } else {
                    match validate(&candidate_source.source) {
                        Ok(proof) => proof,
                        Err(error) => {
                            let fallback = error.to_string();
                            diagnostics = error
                                .diagnostics
                                .into_iter()
                                .take(MAXIMUM_PATCHBAY_DIAGNOSTICS)
                                .collect();
                            if diagnostics.is_empty() {
                                diagnostics.push(fallback);
                            }
                            CompatibilityProof {
                                compatible: false,
                                code: error.code.to_owned(),
                                producer_type: None,
                                consumer_type: None,
                                candidate_plan_identity: None,
                                plan_disposition: "unavailable".to_owned(),
                            }
                        }
                    }
                }
            } else {
                if candidate_source.semantic_hash.is_none() {
                    return Err(ProtocolError {
                        code: "CND-PBY-004",
                        message: "typed source edit did not parse; transaction was not applied"
                            .to_owned(),
                        diagnostics: document
                            .diagnostics
                            .iter()
                            .take(MAXIMUM_PATCHBAY_DIAGNOSTICS)
                            .map(ToString::to_string)
                            .collect(),
                        disposition: EditDisposition::Rejected,
                    });
                }
                validate(&candidate_source.source)?
            }
        } else {
            CompatibilityProof {
                compatible: true,
                code: "CND-PBY-PRESENTATION-ONLY".to_owned(),
                producer_type: None,
                consumer_type: None,
                candidate_plan_identity: None,
                plan_disposition: "not-applicable".to_owned(),
            }
        };
        if source_changed {
            candidate_source.revision += 1;
        }
        let presentation_revision = self.presentation.revision + u64::from(presentation_changed);
        let candidate_presentation =
            presentation_snapshot(&self.source.document_id, presentation_revision, positions);
        self.source = candidate_source;
        self.presentation = candidate_presentation;
        self.history.push_back(WorkspaceRevision {
            source: self.source.clone(),
            presentation: self.presentation.clone(),
        });
        while self.history.len() > self.history_limit {
            self.history.pop_front();
        }
        Ok(EditResult {
            source: self.source.clone(),
            presentation: self.presentation.clone(),
            semantic: self.semantic_with_lookup(lookup),
            candidate_revision: CandidateRevision {
                source: self.source.revision,
                presentation: self.presentation.revision,
            },
            diagnostics,
            compatibility,
            disposition: EditDisposition::Committed,
        })
    }
}

fn rejected(code: &'static str, message: &str) -> ProtocolError {
    rejected_with_diagnostics(code, message, Vec::new())
}

fn rejected_with_diagnostics(
    code: &'static str,
    message: &str,
    diagnostics: Vec<String>,
) -> ProtocolError {
    ProtocolError {
        code,
        message: message.to_owned(),
        diagnostics: diagnostics
            .into_iter()
            .take(MAXIMUM_PATCHBAY_DIAGNOSTICS)
            .collect(),
        disposition: EditDisposition::Rejected,
    }
}

fn validate_cord_bounds(bounds: &CordEditBounds) -> Result<(), ProtocolError> {
    if bounds.capacity_items == 0
        || bounds.max_value_bytes == 0
        || bounds.max_queued_bytes
            < u64::from(bounds.capacity_items) * u64::from(bounds.max_value_bytes)
        || bounds.high_watermark_items == 0
        || bounds.high_watermark_items > bounds.capacity_items
        || bounds.low_watermark_items >= bounds.high_watermark_items
        || bounds.pressure != "block"
    {
        return Err(rejected(
            "CND-PBY-010",
            "connection requires valid finite bounds and a supported pressure contract",
        ));
    }
    Ok(())
}

fn canonical_cord_source(
    from_node: &str,
    from_port: &str,
    to_node: &str,
    to_port: &str,
    bounds: &CordEditBounds,
) -> String {
    format!(
        "\ncord {from_node}.{from_port} -> {to_node}.{to_port} {{\n    capacity = {}\n    max_value_bytes = {}\n    max_queued_bytes = {}\n    low_watermark = {}\n    high_watermark = {}\n    pressure = {}\n}}\n",
        bounds.capacity_items,
        bounds.max_value_bytes,
        bounds.max_queued_bytes,
        bounds.low_watermark_items,
        bounds.high_watermark_items,
        bounds.pressure
    )
}

fn canonical_edit_value(value: &EditValue) -> String {
    match value {
        EditValue::Boolean(value) => value.to_string(),
        EditValue::Integer(value) => value.to_string(),
        EditValue::Text(value) => format!("{value:?}"),
        EditValue::Reference(value) => format!("ref({value:?})"),
        EditValue::ContractReference(value) => format!("contract({value:?})"),
        EditValue::ExactDecimal(value) => format!("decimal({value:?})"),
    }
}

fn source_span_offsets(source: &str, span: conduit_panel::SourceSpan) -> Option<(usize, usize)> {
    fn offset(source: &str, line: usize, column: usize) -> Option<usize> {
        if line == 0 || column == 0 {
            return None;
        }
        let line_start = source
            .split_inclusive('\n')
            .take(line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>();
        let line_text = source.get(line_start..)?.split('\n').next()?;
        let column_offset = line_text
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(line_text.len(), |(index, _)| index);
        Some(line_start + column_offset)
    }
    let start = offset(source, span.line, span.column)?;
    let end = offset(source, span.end_line, span.end_column)?;
    (start <= end && end <= source.len()).then_some((start, end))
}

fn replace_source_span(
    source: &mut String,
    span: conduit_panel::SourceSpan,
    replacement: &str,
) -> Result<(), ProtocolError> {
    let (start, end) = source_span_offsets(source, span)
        .ok_or_else(|| rejected("CND-PBY-012", "invalid configuration source span"))?;
    source.replace_range(start..end, replacement);
    Ok(())
}

fn remove_source_span(
    source: &mut String,
    span: conduit_panel::SourceSpan,
) -> Result<(), ProtocolError> {
    let whole_declaration = conduit_panel::SourceSpan { column: 1, ..span };
    let (start, mut end) = source_span_offsets(source, whole_declaration)
        .ok_or_else(|| rejected("CND-PBY-012", "invalid cord source span"))?;
    if source.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    source.replace_range(start..end, "");
    Ok(())
}

fn presentation_snapshot(
    document_id: &str,
    revision: u64,
    node_positions: BTreeMap<String, NodePosition>,
) -> PresentationSnapshot {
    let mut identity_input = format!("conduit.patchbay-presentation\0{document_id}\0{revision}\0");
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

fn exact_source_identity(source: &str) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest([b"conduit.patchbay-source\0".as_slice(), source.as_bytes()].concat())
    )
}
