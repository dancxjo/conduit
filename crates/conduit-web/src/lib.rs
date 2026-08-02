//! Safe browser bindings to the production `.panel` parser.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
};

use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

use conduit_compile::{
    CompileInput, EvidenceProviderBindingDocument, InstalledHostObservationInput, InstalledProfile,
    PinDocument, PlanArtifactDocument, PlanHostDocument, WatchAdmissionDocument, compile_source,
};
use conduit_core::{
    ArtifactDigest, EvidenceCursorStatus, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION,
    SchedulerPolicy, SemanticHash, TerminalClass, classify_evidence_cursor,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, ExactEvidenceBatch, ExactEvidenceCommitReceipt,
    ExactEvidenceCommitRequest, ExactEvidenceDrainError, ExactEvidenceProvider,
    ExactEvidenceProviderBinding, ExactEvidenceRecord, ExactEvidenceUseAuthority,
    ExactExecutionReport, ExactHostedRunSession, ExactHostedServiceUseObservation, ExactRunContext,
    ExactRunIo, ExactRunSessionRegistry, ExactRunState, ExactWatchBatch, ExactWatchMaterial,
    ExactWatchObservation, ExactWatchOperation, ExactWatchSubject, ExactWatchUsage,
    ExactWatchUseAuthority, Handler, Registry, ResolutionError, RunIo, RuntimeError,
    SchedulerReservation, Value, exact_evidence_provider_binding, file_read_contract,
    file_watch_contract, file_write_contract, hosted_service_use_observations,
};
use conduit_std::{
    FileHandle, FileSlot, FlushClaim, MemoryFilesystem, PartialWritePolicy, ReadConsistency,
    ReadRequest, WatchCoalescing, WatchOverflow, WatchRequest, WriteMode, WriteRequest,
};

const MAXIMUM_PATCHBAY_SESSIONS: usize = 8;
const MAXIMUM_PATCHBAY_SESSION_ID_BYTES: usize = 256;
const MAXIMUM_PATCHBAY_REQUEST_BYTES: usize = 1024 * 1024;
const MAXIMUM_BROWSER_ACTIVE_RUN_MEMORY_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_BROWSER_RUN_PUMP_DECISIONS: u64 = 256;
const MAXIMUM_BROWSER_WATCH_PREVIEW_BYTES: u32 = 256;
const MAXIMUM_BROWSER_EVIDENCE_DRAIN_EVENTS: u32 = 128;
const MAXIMUM_BROWSER_RETAINED_EVIDENCE_EVENTS: usize = 256;
const MAXIMUM_PATCHBAY_PROJECTED_EVIDENCE_EVENTS: usize = 32;
const BROWSER_READ_RESOURCE: &str = "conduit.resource/filesystem-example-read";
const BROWSER_WRITE_RESOURCE: &str = "conduit.resource/filesystem-example-write";
const BROWSER_WATCH_RESOURCE: &str = "conduit.resource/filesystem-example-watch";
const BROWSER_EVIDENCE_IMPLEMENTATION: &str = "conduit/browser-worker-exact-evidence";
const BROWSER_EVIDENCE_ARTIFACT: &str = "conduit/browser-worker-exact-evidence-artifact";
const BROWSER_EVIDENCE_STORE: &str = "conduit.resource/browser-exact-evidence";
const BROWSER_EVIDENCE_LEASE: &str = "conduit.lease/browser-exact-evidence";
const BROWSER_EVIDENCE_HOST_OBSERVATION: &str = "conduit/browser-worker-evidence-host-observation";
const BROWSER_EVIDENCE_HOST: &str = "conduit/browser-worker";
const BROWSER_WATCH_OPERATOR: &str = "operator/browser-patchbay";
const BROWSER_FILE_BYTES: &[u8] = b"bounded filesystem fixture\n";
const MONOTONIC_CLOCK_HASH: &[u8; 32] = &[
    0x6b, 0x9c, 0x68, 0x72, 0x26, 0xd4, 0xa1, 0x96, 0x5e, 0x78, 0x0b, 0x63, 0xb4, 0xbd, 0xc0, 0x92,
    0x2d, 0xe2, 0xa6, 0x86, 0xc3, 0xc1, 0x36, 0x5f, 0x4f, 0x68, 0xf7, 0x21, 0x9f, 0x30, 0xcc, 0x48,
];

fn browser_host_observation() -> InstalledHostObservationInput {
    let mut observation = InstalledHostObservationInput::conduct_host();
    observation.id = "conduit/browser-worker-host-observation".to_owned();
    observation.host = BROWSER_EVIDENCE_HOST.to_owned();
    observation.time_basis = "clock/browser-worker".to_owned();
    observation
}

fn browser_evidence_hash(domain: &[u8], facts: &[&[u8]]) -> SemanticHash {
    let mut hasher = Sha256::new();
    hasher.update(b"conduit.browser-evidence-observation\0");
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for fact in facts {
        hasher.update((fact.len() as u64).to_be_bytes());
        hasher.update(fact);
    }
    SemanticHash::from_bytes(hasher.finalize().into())
}

fn browser_exact_run_id(session_id: &str, source_revision: u64) -> String {
    let identity = browser_evidence_hash(
        b"exact-run-id",
        &[session_id.as_bytes(), &source_revision.to_be_bytes()],
    )
    .to_string();
    let digest = identity
        .strip_prefix("sha256:")
        .expect("semantic hashes use the canonical sha256 prefix");
    format!("run-{}", &digest[..32])
}

fn browser_evidence_provider_observation() -> EvidenceProviderBindingDocument {
    // This artifact is the exact source compiled into the current provider,
    // rather than a placeholder digest. The generated WASM/browser-plan gate
    // separately binds the deployed binary bytes.
    let artifact_digest =
        ArtifactDigest::from_bytes(Sha256::digest(include_bytes!("lib.rs")).into());
    let implementation_identity = browser_evidence_hash(
        b"implementation",
        &[
            BROWSER_EVIDENCE_IMPLEMENTATION.as_bytes(),
            artifact_digest.as_bytes(),
        ],
    );
    let grant_hash = browser_evidence_hash(
        b"grant",
        &[
            BROWSER_EVIDENCE_STORE.as_bytes(),
            b"commit-exact-evidence",
            b"clock/browser-worker",
        ],
    );
    let host_semantic_hash = browser_evidence_hash(
        b"host-observation",
        &[
            BROWSER_EVIDENCE_HOST.as_bytes(),
            BROWSER_EVIDENCE_STORE.as_bytes(),
            artifact_digest.as_bytes(),
            &1_u64.to_be_bytes(),
        ],
    );
    EvidenceProviderBindingDocument {
        implementation: PinDocument {
            id: BROWSER_EVIDENCE_IMPLEMENTATION.to_owned(),
            schema_version: 0,
            semantic_hash: implementation_identity.to_string(),
        },
        artifact: PlanArtifactDocument {
            id: BROWSER_EVIDENCE_ARTIFACT.to_owned(),
            digest: artifact_digest.to_string(),
        },
        host_observation: PlanHostDocument {
            id: BROWSER_EVIDENCE_HOST_OBSERVATION.to_owned(),
            host: BROWSER_EVIDENCE_HOST.to_owned(),
            semantic_hash: host_semantic_hash.to_string(),
            time_basis: "clock/browser-worker".to_owned(),
            observed_at_tick: 12,
            valid_until_tick: u64::MAX,
        },
        store_kind: "evidence-store".to_owned(),
        store_id: BROWSER_EVIDENCE_STORE.to_owned(),
        store_generation: 1,
        grant_hash: grant_hash.to_string(),
        time_basis: "clock/browser-worker".to_owned(),
    }
}

fn browser_evidence_authority(
    binding: &ExactEvidenceProviderBinding,
    run_id: &str,
    plan_epoch: u64,
) -> ExactEvidenceUseAuthority {
    ExactEvidenceUseAuthority {
        grant_hash: binding.grant_hash,
        grant_active: true,
        run_id: run_id.to_owned(),
        plan_epoch,
        host_observation_id: binding.host_observation_id.clone(),
        store_resource_kind: binding.store_resource_kind.clone(),
        store_resource_id: binding.store_resource_id.clone(),
        store_generation: binding.store_generation,
        lease_id: BROWSER_EVIDENCE_LEASE.to_owned(),
        lease_epoch: plan_epoch,
        lease_available: true,
        time_basis: binding.time_basis.clone(),
        validated_at_tick: 12,
        valid_until_tick: u64::MAX,
    }
}

thread_local! {
    static PATCHBAY_SESSIONS: RefCell<BTreeMap<String, BrowserPatchbaySession>> =
        const { RefCell::new(BTreeMap::new()) };
    static BROWSER_RUN_SESSIONS: RefCell<Option<ExactRunSessionRegistry>> =
        const { RefCell::new(None) };
}

struct ExactBrowserResult {
    report: ExactExecutionReport,
    output: Vec<u8>,
    error: Vec<u8>,
    display: Vec<u8>,
    patchbay: serde_json::Value,
}

/// Browser-worker-owned exact-run state. The compiler arena, source document,
/// and registry are not retained: `ExactHostedRunSession` owns only the
/// admitted runtime state that was pinned at Start.
struct BrowserExactRun {
    plan: conduit_patchbay::PlanSnapshot,
    run_id: String,
    source_revision: u64,
    node_count: usize,
    cord_count: usize,
    session: Option<ExactHostedRunSession>,
    use_observations: Vec<ExactHostedServiceUseObservation>,
    watch_admissions: Vec<BrowserWatchAdmission>,
    evidence: Rc<RefCell<BrowserEvidenceStore>>,
    terminal: Option<BrowserExactRunTerminal>,
}

/// Worker-owned committed evidence for the rolling browser service profile.
/// Patchbay may read this bounded projection but cannot acknowledge scheduler
/// storage or become the authoritative sink.
#[derive(Clone)]
struct BrowserEvidenceStore {
    records: VecDeque<(ExactEvidenceRecord, u64)>,
    earliest_cursor: u64,
    next_cursor: u64,
    retained_bytes: u64,
    high_water_events: usize,
    high_water_bytes: u64,
    dropped_events: u64,
    maximum_bytes: u64,
    committed: BTreeMap<(u64, u64), ExactEvidenceCommitReceipt>,
}

impl BrowserEvidenceStore {
    fn new(maximum_bytes: u64) -> Result<Self, RuntimeError> {
        if maximum_bytes == 0 {
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider requires a positive plan evidence-byte budget",
            ));
        }
        Ok(Self {
            records: VecDeque::new(),
            earliest_cursor: 0,
            next_cursor: 0,
            retained_bytes: 0,
            high_water_events: 0,
            high_water_bytes: 0,
            dropped_events: 0,
            maximum_bytes,
            committed: BTreeMap::new(),
        })
    }

    fn commit_through(&mut self, cursor: u64) -> Result<(), RuntimeError> {
        if cursor < self.next_cursor {
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider cursor reversed",
            ));
        }
        self.next_cursor = cursor;
        if self.dropped_events != 0 {
            self.earliest_cursor = self
                .records
                .front()
                .map_or(cursor, |(record, _)| record.sequence);
        }
        Ok(())
    }

    fn read(&self, cursor: u64, maximum_events: u32) -> Result<ExactEvidenceBatch, RuntimeError> {
        let status = classify_evidence_cursor(cursor, self.earliest_cursor, self.next_cursor)
            .map_err(|error| RuntimeError::new("CND-PBY-009", error.to_string()))?;
        let start = match status {
            EvidenceCursorStatus::Available => cursor,
            EvidenceCursorStatus::Gap { resume_at } => resume_at,
            EvidenceCursorStatus::Future { next_sequence } => next_sequence,
        };
        let records = if status == EvidenceCursorStatus::Available {
            self.records
                .iter()
                .filter(|(record, _)| record.sequence >= start)
                .take(usize::try_from(maximum_events).expect("u32 fits usize"))
                .map(|(record, _)| record.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let next_cursor = match status {
            EvidenceCursorStatus::Available => records.last().map_or(self.next_cursor, |record| {
                record
                    .sequence
                    .checked_add(1)
                    .expect("exact evidence cursor cannot overflow")
            }),
            EvidenceCursorStatus::Gap { resume_at } => resume_at,
            EvidenceCursorStatus::Future { next_sequence } => next_sequence,
        };
        Ok(ExactEvidenceBatch {
            status,
            next_cursor,
            records,
        })
    }

    fn usage(&self) -> serde_json::Value {
        serde_json::json!({
            "earliest_cursor": self.earliest_cursor,
            "next_cursor": self.next_cursor,
            "retained_events": self.records.len(),
            "retained_bytes": self.retained_bytes,
            "high_water_events": self.high_water_events,
            "high_water_bytes": self.high_water_bytes,
            "dropped_events": self.dropped_events,
            "maximum_events": MAXIMUM_BROWSER_RETAINED_EVIDENCE_EVENTS,
            "maximum_bytes": self.maximum_bytes,
        })
    }
}

struct BrowserEvidenceProvider {
    binding: ExactEvidenceProviderBinding,
    store: Rc<RefCell<BrowserEvidenceStore>>,
    authority: Rc<RefCell<Option<ExactEvidenceUseAuthority>>>,
}

impl ExactEvidenceProvider for BrowserEvidenceProvider {
    fn binding(&self) -> &ExactEvidenceProviderBinding {
        &self.binding
    }

    fn observe_use_authority(
        &self,
        _run: &conduit_runtime::ExactRunIdentity,
    ) -> Result<ExactEvidenceUseAuthority, RuntimeError> {
        self.authority.borrow().clone().ok_or_else(|| {
            RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider authority is unavailable at use time",
            )
        })
    }

    fn commit_exact_evidence(
        &mut self,
        request: &ExactEvidenceCommitRequest,
        records: &[ExactEvidenceRecord],
    ) -> Result<ExactEvidenceCommitReceipt, RuntimeError> {
        if request.provider != self.binding {
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider rejected exact binding drift",
            ));
        }
        let key = (request.start_cursor, request.end_cursor);
        let mut store = self.store.borrow_mut();
        if let Some(receipt) = store.committed.get(&key) {
            if receipt.batch_digest == request.batch_digest {
                return Ok(receipt.clone());
            }
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider rejected a changed idempotent batch",
            ));
        }
        if request.start_cursor != store.next_cursor || request.end_cursor <= request.start_cursor {
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence provider rejected cursor drift",
            ));
        }

        // Validate and apply to a complete staged store. No visible state is
        // changed unless every record and every accounting operation succeeds.
        let mut staged = store.clone();
        for record in records {
            if record.plan_identity != request.plan_identity.to_string()
                || record.plan_epoch != request.plan_epoch
                || record.run_id != request.run_id
                || record.sequence < request.start_cursor
                || record.sequence >= request.end_cursor
            {
                return Err(RuntimeError::new(
                    "CND-PBY-009",
                    "browser evidence provider rejected identity or cursor drift",
                ));
            }
            let bytes = u64::try_from(
                serde_json::to_vec(record)
                    .map_err(|error| RuntimeError::new("CND-PBY-009", error.to_string()))?
                    .len(),
            )
            .expect("usize fits u64");
            if bytes > staged.maximum_bytes {
                return Err(RuntimeError::new(
                    "CND-PBY-009",
                    "one exact evidence record exceeds the browser provider byte bound",
                ));
            }
            staged.records.push_back((record.clone(), bytes));
            staged.retained_bytes = staged
                .retained_bytes
                .checked_add(bytes)
                .ok_or_else(|| RuntimeError::new("CND-PBY-009", "evidence byte overflow"))?;
            while staged.records.len() > MAXIMUM_BROWSER_RETAINED_EVIDENCE_EVENTS
                || staged.retained_bytes > staged.maximum_bytes
            {
                let (evicted, evicted_bytes) = staged
                    .records
                    .pop_front()
                    .expect("an over-bound evidence store is nonempty");
                staged.retained_bytes -= evicted_bytes;
                staged.dropped_events = staged
                    .dropped_events
                    .checked_add(1)
                    .ok_or_else(|| RuntimeError::new("CND-PBY-009", "evidence gap overflow"))?;
                staged.earliest_cursor = evicted
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| RuntimeError::new("CND-PBY-009", "evidence cursor overflow"))?;
            }
            staged.high_water_events = staged.high_water_events.max(staged.records.len());
            staged.high_water_bytes = staged.high_water_bytes.max(staged.retained_bytes);
        }
        staged.commit_through(request.end_cursor)?;
        let receipt = ExactEvidenceCommitReceipt::acknowledged(request);
        staged.committed.insert(key, receipt.clone());
        *store = staged;
        Ok(receipt)
    }
}

#[derive(Clone)]
struct BrowserWatchAdmission {
    id: String,
    maximum_history: u32,
    operator: String,
    control_grant_hash: SemanticHash,
    control_grant_active: bool,
    lease: String,
    lease_available: bool,
    reveal_grant_hash: Option<SemanticHash>,
    reveal_grant_active: bool,
    time_basis: String,
    valid_until_tick: u64,
}

fn browser_watch_use_authority(
    run: &BrowserExactRun,
    operator_id: &str,
    watch_id: &str,
    operation: ExactWatchOperation,
) -> Result<ExactWatchUseAuthority, RuntimeError> {
    let admission = run
        .watch_admissions
        .iter()
        .find(|watch| watch.id == watch_id)
        .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch is not admitted by this plan"))?;
    if operator_id != admission.operator
        || !admission.control_grant_active
        || !admission.lease_available
        || (admission.reveal_grant_hash.is_some() && !admission.reveal_grant_active)
        || 12 >= admission.valid_until_tick
    {
        return Err(RuntimeError::new(
            "CND-WAT-004",
            "browser Watch operator, grant, reveal, lease, or time observation is not current",
        ));
    }
    Ok(ExactWatchUseAuthority {
        operation,
        operator_id: operator_id.to_owned(),
        control_grant_hash: admission.control_grant_hash,
        control_grant_active: admission.control_grant_active,
        run_id: run.run_id.clone(),
        plan_epoch: run.source_revision,
        watch_id: watch_id.to_owned(),
        lease_id: admission.lease.clone(),
        lease_epoch: run.source_revision,
        lease_available: admission.lease_available,
        reveal_grant_hash: admission.reveal_grant_hash,
        reveal_grant_active: admission.reveal_grant_active,
        time_basis: admission.time_basis.clone(),
        validated_at_tick: 12,
        valid_until_tick: admission.valid_until_tick,
    })
}

/// The finite terminal projection retained after the executor and its session
/// admission have been released. This is not a second runtime or an evidence
/// store: it is the bounded final observation already emitted by that run.
struct BrowserExactRunTerminal {
    state: ExactRunState,
    high_water: conduit_runtime::SchedulerHighWater,
    watches: BTreeMap<String, ExactWatchBatch>,
    output: Vec<u8>,
    error: Vec<u8>,
    display: Vec<u8>,
}

/// One revisioned authoring workspace plus, at most, one separately pinned
/// active exact run. Candidate source edits mutate only `workspace`.
struct BrowserPatchbaySession {
    workspace: conduit_patchbay::Workspace,
    run: Option<BrowserExactRun>,
}

fn browser_filesystem() -> MemoryFilesystem<1, 256, 8> {
    MemoryFilesystem::new([FileSlot::seeded(FileHandle(1), BROWSER_FILE_BYTES, false)
        .expect("browser filesystem fixture is statically bounded")])
}

fn exact_keys(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() == expected.len()
        && expected
            .iter()
            .all(|key| node.config.iter().any(|entry| entry.key == *key))
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-FSH-019",
            format!(
                "browser file node `{}` has an incomplete exact config",
                node.id
            ),
        ))
    }
}

fn exact_secret(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    )
}

fn exact_usize(node: &Node, key: &str, maximum: usize) -> Result<usize, RuntimeError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) if *value > 0 && *value <= maximum as i128 => {
            usize::try_from(*value)
                .map_err(|_| RuntimeError::new("CND-FS-001", "file bound does not fit usize"))
        }
        _ => Err(RuntimeError::new(
            "CND-FS-001",
            format!("browser file node `{}` has invalid `{key}`", node.id),
        )),
    }
}

fn validate_browser_read(node: &Node) -> Result<(), ResolutionError> {
    exact_keys(
        node,
        &[
            "resource",
            "grant",
            "offset",
            "maximum_bytes",
            "chunk_bytes",
            "consistency",
            "eof",
            "cancellation",
        ],
    )?;
    if exact_secret(node, "resource", BROWSER_READ_RESOURCE)
        && exact_secret(node, "grant", "conduit.grant/filesystem-read")
        && matches!(node.config("consistency"), Some("snapshot" | "live"))
        && node.config("eof") == Some("terminal")
        && node.config("cancellation") == Some("discard")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-FSH-019",
            "browser read requests unsupported resource or semantics",
        ))
    }
}

fn validate_browser_write(node: &Node) -> Result<(), ResolutionError> {
    exact_keys(
        node,
        &[
            "resource",
            "grant",
            "mode",
            "maximum_bytes",
            "partial",
            "flush",
            "cleanup",
            "cancellation",
        ],
    )?;
    if exact_secret(node, "resource", BROWSER_WRITE_RESOURCE)
        && exact_secret(node, "grant", "conduit.grant/filesystem-write")
        && matches!(node.config("mode"), Some("create" | "replace" | "append"))
        && matches!(
            node.config("partial"),
            Some("fail-without-commit" | "report-committed-prefix")
        )
        && matches!(node.config("flush"), Some("none" | "provider-accepted"))
        && node.config("cleanup") == Some("close")
        && node.config("cancellation") == Some("close")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-FSH-019",
            "browser write requests unsupported resource or semantics",
        ))
    }
}

fn validate_browser_watch(node: &Node) -> Result<(), ResolutionError> {
    exact_keys(
        node,
        &[
            "resource",
            "grant",
            "clock",
            "clock_schema_version",
            "clock_hash",
            "event_kinds",
            "emit_initial",
            "coalescing",
            "loss",
            "queue_capacity",
            "maximum_events",
            "overflow",
            "rename_identity",
            "cancellation",
        ],
    )?;
    if exact_secret(node, "resource", BROWSER_WATCH_RESOURCE)
        && exact_secret(node, "grant", "conduit.grant/filesystem-watch")
        && node.config("clock") == Some("conduit.clock/monotonic-ticks")
        && matches!(
            node.config_value("clock_schema_version"),
            Some(SourceValue::Integer(0))
        )
        && matches!(
            node.config_value("clock_hash"),
            Some(SourceValue::Bytes(hash)) if hash.as_slice() == MONOTONIC_CLOCK_HASH
        )
        && node.config("event_kinds") == Some("create-change-remove-rename")
        && matches!(
            node.config_value("emit_initial"),
            Some(SourceValue::Boolean(true))
        )
        && matches!(
            node.config("coalescing"),
            Some("none" | "same-handle-latest")
        )
        && node.config("loss") == Some("explicit-gap")
        && matches!(node.config("overflow"), Some("terminal-gap" | "gap-resync"))
        && node.config("rename_identity") == Some("preserve-handle")
        && node.config("cancellation") == Some("close")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-FSH-019",
            "browser watch requests unsupported resource or semantics",
        ))
    }
}

struct BrowserRead;

impl Handler for BrowserRead {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new("CND-FSH-019", "read has hidden inputs"));
        }
        let offset = match node.config_value("offset") {
            Some(SourceValue::Integer(value)) if *value >= 0 => *value as u64,
            _ => return Err(RuntimeError::new("CND-FS-001", "invalid read offset")),
        };
        let maximum_bytes = exact_usize(node, "maximum_bytes", 256)?;
        let chunk_bytes = exact_usize(node, "chunk_bytes", 256)?;
        let mut output = [0; 256];
        let result = browser_filesystem()
            .read(
                ReadRequest {
                    handle: FileHandle(1),
                    offset,
                    maximum_bytes,
                    chunk_bytes,
                    consistency: if node.config("consistency") == Some("live") {
                        ReadConsistency::Live
                    } else {
                        ReadConsistency::Snapshot
                    },
                },
                &mut output[..chunk_bytes],
            )
            .map_err(|error| RuntimeError::new(error.code(), error.code()))?;
        let mut metadata = Vec::with_capacity(25);
        metadata.extend_from_slice(&(result.bytes_read as u64).to_be_bytes());
        metadata.extend_from_slice(&result.next_offset.to_be_bytes());
        metadata.extend_from_slice(&result.generation.to_be_bytes());
        metadata.push(u8::from(result.eof));
        Ok(vec![
            Value {
                value_type: file_read_contract().outputs[0].value_type,
                bytes: output[..result.bytes_read].to_vec(),
            },
            Value {
                value_type: file_read_contract().outputs[1].value_type,
                bytes: metadata,
            },
        ])
    }
}

struct BrowserWrite;

impl Handler for BrowserWrite {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == file_write_contract().inputs[0].value_type)
            .ok_or_else(|| RuntimeError::new("CND-FSH-019", "write chunk is missing"))?;
        let mode = match node.config("mode") {
            Some("create") => WriteMode::Create,
            Some("append") => WriteMode::Append,
            _ => WriteMode::Replace,
        };
        let partial = if node.config("partial") == Some("report-committed-prefix") {
            PartialWritePolicy::ReportCommittedPrefix
        } else {
            PartialWritePolicy::FailWithoutCommit
        };
        let flush = if node.config("flush") == Some("provider-accepted") {
            FlushClaim::ProviderAccepted
        } else {
            FlushClaim::None
        };
        let result = browser_filesystem()
            .write(
                WriteRequest {
                    handle: FileHandle(1),
                    mode,
                    maximum_bytes: exact_usize(node, "maximum_bytes", 256)?,
                    partial,
                    requested_flush: flush,
                },
                &input.bytes,
            )
            .map_err(|error| RuntimeError::new(error.code(), error.code()))?;
        let mut metadata = Vec::with_capacity(19);
        metadata.extend_from_slice(&(result.bytes_written as u64).to_be_bytes());
        metadata.extend_from_slice(&result.generation.to_be_bytes());
        metadata.push(u8::from(result.committed));
        metadata.push(u8::from(result.complete));
        metadata.push(match result.flush {
            FlushClaim::None => 0,
            FlushClaim::ProviderAccepted => 1,
            FlushClaim::Durable => 2,
        });
        Ok(vec![Value {
            value_type: file_write_contract().outputs[0].value_type,
            bytes: metadata,
        }])
    }
}

struct BrowserWatch;

impl Handler for BrowserWatch {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new("CND-FSH-019", "watch has hidden inputs"));
        }
        let mut filesystem = browser_filesystem();
        filesystem
            .begin_watch(WatchRequest {
                emit_initial: true,
                maximum_events: exact_usize(node, "maximum_events", 8)?,
                queue_capacity: exact_usize(node, "queue_capacity", 8)?,
                coalescing: if node.config("coalescing") == Some("same-handle-latest") {
                    WatchCoalescing::SameHandleLatest
                } else {
                    WatchCoalescing::None
                },
                overflow: if node.config("overflow") == Some("terminal-gap") {
                    WatchOverflow::TerminalGap
                } else {
                    WatchOverflow::GapThenResync
                },
            })
            .map_err(|error| RuntimeError::new(error.code(), error.code()))?;
        let event = filesystem
            .take_watch_event()
            .map_err(|error| RuntimeError::new(error.code(), error.code()))?
            .ok_or_else(|| RuntimeError::new("CND-FS-009", "initial watch event disappeared"))?;
        let resource = BROWSER_WATCH_RESOURCE.as_bytes();
        let mut bytes = Vec::with_capacity(11 + resource.len());
        bytes.push(0);
        bytes.extend_from_slice(&event.generation.to_be_bytes());
        bytes.extend_from_slice(&(resource.len() as u16).to_be_bytes());
        bytes.extend_from_slice(resource);
        Ok(vec![Value {
            value_type: file_watch_contract().outputs[0].value_type,
            bytes,
        }])
    }
}

fn browser_registry() -> Registry {
    static READ_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x31; 32])];
    static WRITE_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x32; 32])];
    static WATCH_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x33; 32])];
    let mut registry = Registry::hosted_primitives();
    conduit_media::register_deterministic_media_providers(&mut registry)
        .expect("deterministic media providers have distinct identities");
    conduit_media::register_deterministic_signal_providers(&mut registry)
        .expect("deterministic signal providers have distinct identities");
    conduit_audio::transform_implementations::install_audio_gain_implementation(
        &mut registry,
        conduit_audio::transform_implementations::ObservedMediaArtifact::browser_wasm_linked(
            include_bytes!("../../conduit-audio/src/transform_implementations.rs"),
            10,
            20,
        )
        .expect("browser WASM-linked media artifact is observable"),
    )
    .expect("browser WASM-linked media provider has a distinct identity");
    conduit_media::register_deterministic_audio_processing_providers(&mut registry)
        .expect("deterministic audio-processing providers have distinct identities");
    conduit_media::register_deterministic_codec_providers(&mut registry)
        .expect("deterministic codec providers have distinct identities");
    conduit_learned::register_deterministic_inference_provider(&mut registry)
        .expect("deterministic inference providers have distinct identities");
    conduit_learned::lifecycle::register_deterministic_training_provider(&mut registry)
        .expect("deterministic learned lifecycle providers have distinct identities");
    conduit_knowledge::register_deterministic_retrieval_provider(&mut registry)
        .expect("deterministic retrieval providers have distinct identities");
    conduit_knowledge::register_deterministic_graph_provider(&mut registry)
        .expect("deterministic graph providers have distinct identities");
    conduit_spatial::register_deterministic_spatial_provider(&mut registry)
        .expect("deterministic spatial providers have distinct identities");
    conduit_spatial::register_deterministic_spatial_data_provider(&mut registry)
        .expect("deterministic spatial-data providers have distinct identities");
    conduit_net::register_deterministic_network_fixture_providers(&mut registry)
        .expect("deterministic network fixture providers have distinct identities");
    for provider in [
        CompiledInHostService {
            contract: file_read_contract(),
            implementation_id: "conduit/filesystem-memory-read",
            artifact_id: "conduit/filesystem-memory-read-artifact",
            entrypoint: "filesystem-memory-read",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &READ_AUTHORITIES,
            factory: || Box::new(BrowserRead),
            validate_config: validate_browser_read,
        },
        CompiledInHostService {
            contract: file_write_contract(),
            implementation_id: "conduit/filesystem-memory-write",
            artifact_id: "conduit/filesystem-memory-write-artifact",
            entrypoint: "filesystem-memory-write",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &WRITE_AUTHORITIES,
            factory: || Box::new(BrowserWrite),
            validate_config: validate_browser_write,
        },
        CompiledInHostService {
            contract: file_watch_contract(),
            implementation_id: "conduit/filesystem-memory-watch",
            artifact_id: "conduit/filesystem-memory-watch-artifact",
            entrypoint: "filesystem-memory-watch",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &WATCH_AUTHORITIES,
            factory: || Box::new(BrowserWatch),
            validate_config: validate_browser_watch,
        },
    ] {
        registry
            .register_compiled_in_host_service(provider)
            .expect("browser filesystem provider identities are unique");
    }
    conduit_cache::register_hosted_cache_provider(&mut registry)
        .expect("browser storage-cache provider identities are unique");
    registry
}

const fn terminal_name(terminal: TerminalClass) -> &'static str {
    match terminal {
        TerminalClass::Succeeded => "succeeded",
        TerminalClass::Disconnected => "disconnected",
        TerminalClass::Cancelled => "cancelled",
        TerminalClass::Failed => "failed",
    }
}

fn patchbay_error(error: conduit_patchbay::ProtocolError) -> String {
    serde_json::json!({
        "ok": false,
        "diagnostic": error.to_string(),
        "code": error.code,
        "diagnostics": error.diagnostics,
        "disposition": error.disposition,
    })
    .to_string()
}

fn patchbay_result(result: conduit_patchbay::EditResult) -> String {
    serde_json::json!({
        "ok": true,
        "source_revision": result.source.revision,
        "presentation_revision": result.presentation.revision,
        "semantic_hash": result.semantic.source_semantic_hash,
        "presentation_identity": result.presentation.identity,
        "positions": result.presentation.node_positions,
        "candidate_revision": result.candidate_revision,
        "diagnostics": result.diagnostics,
        "compatibility": result.compatibility,
        "disposition": result.disposition,
    })
    .to_string()
}

fn patchbay_rejection(
    error: conduit_patchbay::ProtocolError,
    workspace: &conduit_patchbay::Workspace,
) -> String {
    serde_json::json!({
        "ok": false,
        "diagnostic": error.to_string(),
        "code": error.code,
        "diagnostics": error.diagnostics,
        "candidate_revision": {
            "source": workspace.source().revision,
            "presentation": workspace.presentation().revision,
        },
        "compatibility": {
            "compatible": false,
            "code": error.code,
            "producer_type": null,
            "consumer_type": null,
            "candidate_plan_identity": null,
            "plan_disposition": "rejected",
        },
        "disposition": error.disposition,
    })
    .to_string()
}

fn browser_run_registry() -> Result<ExactRunSessionRegistry, RuntimeError> {
    BROWSER_RUN_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        if sessions.is_none() {
            *sessions = Some(
                ExactRunSessionRegistry::new(
                    MAXIMUM_PATCHBAY_SESSIONS,
                    MAXIMUM_BROWSER_ACTIVE_RUN_MEMORY_BYTES,
                )
                .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?,
            );
        }
        Ok(sessions
            .as_ref()
            .expect("browser exact-run registry was initialized")
            .clone())
    })
}

fn patchbay_high_water(
    high_water: conduit_runtime::SchedulerHighWater,
) -> conduit_patchbay::PatchbayHighWaterProjection {
    conduit_patchbay::PatchbayHighWaterProjection {
        queue_items: high_water.queue_items,
        queue_payload_bytes: high_water.queue_payload_bytes,
        ready_slots: high_water.ready_slots,
        event_slots: high_water.event_slots,
        decisions: high_water.decisions,
    }
}

fn patchbay_run_snapshot(run: &BrowserExactRun) -> conduit_patchbay::RunSnapshot {
    let state = match browser_run_state(run) {
        ExactRunState::Terminal(_) => conduit_patchbay::RunState::Terminal,
        ExactRunState::Active => conduit_patchbay::RunState::Active,
        ExactRunState::Waiting => conduit_patchbay::RunState::Waiting,
        ExactRunState::Quiescing => conduit_patchbay::RunState::Quiescing,
        ExactRunState::Aborting => conduit_patchbay::RunState::Aborting,
    };
    conduit_patchbay::RunSnapshot {
        run_id: run.run_id.clone(),
        plan_identity: run.plan.identity.clone(),
        source_semantic_hash: run.plan.source_semantic_hash.clone(),
        state,
    }
}

fn browser_run_evidence(run: &BrowserExactRun) -> Result<Vec<serde_json::Value>, RuntimeError> {
    browser_run_evidence_records(run)
        .iter()
        .map(|event| {
            serde_json::to_value(event)
                .map_err(|error| RuntimeError::new("CND-PBY-009", error.to_string()))
        })
        .collect()
}

fn browser_run_state(run: &BrowserExactRun) -> ExactRunState {
    run.terminal
        .as_ref()
        .map(|terminal| terminal.state)
        .unwrap_or_else(|| {
            run.session
                .as_ref()
                .expect("live browser run retains its exact session")
                .state()
        })
}

fn browser_run_high_water(run: &BrowserExactRun) -> conduit_runtime::SchedulerHighWater {
    run.terminal
        .as_ref()
        .map(|terminal| terminal.high_water)
        .unwrap_or_else(|| {
            run.session
                .as_ref()
                .expect("live browser run retains its exact session")
                .high_water()
        })
}

fn browser_run_evidence_records(
    run: &BrowserExactRun,
) -> Vec<conduit_runtime::ExactEvidenceRecord> {
    let evidence = run.evidence.borrow();
    let mut records = evidence
        .records
        .iter()
        .rev()
        .take(MAXIMUM_PATCHBAY_PROJECTED_EVIDENCE_EVENTS)
        .map(|(record, _)| record.clone())
        .collect::<Vec<_>>();
    records.reverse();
    records
}

fn browser_exact_evidence_delta(
    run: &BrowserExactRun,
    cursor: u64,
    maximum_events: u32,
) -> Result<conduit_runtime::ExactEvidenceBatch, RuntimeError> {
    run.evidence.borrow().read(cursor, maximum_events)
}

fn drain_browser_exact_evidence(run: &mut BrowserExactRun) -> Result<(), RuntimeError> {
    let BrowserExactRun { session, .. } = run;
    let Some(session) = session.as_mut() else {
        return Ok(());
    };
    while session.retained_event_cursor() < session.next_event_cursor() {
        let cursor = session.retained_event_cursor();
        let batch = session
            .drain_exact_evidence(cursor, MAXIMUM_BROWSER_EVIDENCE_DRAIN_EVENTS)
            .map_err(|error| match error {
                ExactEvidenceDrainError::Scheduler(error) => {
                    RuntimeError::new(error.code(), error.to_string())
                }
                ExactEvidenceDrainError::Provider(error)
                | ExactEvidenceDrainError::Authority(error)
                | ExactEvidenceDrainError::Receipt(error) => error,
            })?;
        if batch.next_cursor <= cursor {
            return Err(RuntimeError::new(
                "CND-PBY-009",
                "browser evidence drain made no cursor progress",
            ));
        }
    }
    Ok(())
}

fn browser_evidence_cursor_status(status: EvidenceCursorStatus) -> serde_json::Value {
    match status {
        EvidenceCursorStatus::Available => serde_json::json!({"kind": "available"}),
        EvidenceCursorStatus::Gap { resume_at } => {
            serde_json::json!({"kind": "gap", "resume_at": resume_at})
        }
        EvidenceCursorStatus::Future { next_sequence } => {
            serde_json::json!({"kind": "future", "next_sequence": next_sequence})
        }
    }
}

fn browser_watch_delta(
    run: &BrowserExactRun,
    watch_id: &str,
    cursor: u64,
    maximum_records: u32,
    authority: &ExactWatchUseAuthority,
) -> Result<ExactWatchBatch, RuntimeError> {
    if let Some(session) = run.session.as_ref() {
        return session.read_watch(watch_id, cursor, maximum_records, authority);
    }
    let retained = run
        .terminal
        .as_ref()
        .expect("browser run has either a live session or terminal snapshot")
        .watches
        .get(watch_id)
        .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch is not admitted by this plan"))?;
    let status = classify_evidence_cursor(cursor, retained.earliest_cursor, retained.next_cursor)
        .map_err(|error| RuntimeError::new("CND-WAT-003", error.to_string()))?;
    let start = match status {
        EvidenceCursorStatus::Available => cursor,
        EvidenceCursorStatus::Gap { resume_at } => resume_at,
        EvidenceCursorStatus::Future { next_sequence } => next_sequence,
    };
    let end = start
        .saturating_add(u64::from(maximum_records))
        .min(retained.next_cursor);
    Ok(ExactWatchBatch {
        status,
        earliest_cursor: retained.earliest_cursor,
        next_cursor: end,
        records: retained
            .records
            .iter()
            .filter(|record| record.cursor >= start && record.cursor < end)
            .cloned()
            .collect(),
    })
}

fn browser_watch_usage(usage: ExactWatchUsage) -> serde_json::Value {
    serde_json::json!({
        "admitted_slots": usage.admitted_slots,
        "attached_slots": usage.attached_slots,
        "retained_observations": usage.retained_observations,
        "retained_preview_bytes": usage.retained_preview_bytes,
        "dropped_observations": usage.dropped_observations,
        "maximum_observations": usage.maximum_observations,
        "maximum_preview_bytes": usage.maximum_preview_bytes,
    })
}

fn browser_watch_observation(record: &ExactWatchObservation) -> serde_json::Value {
    let subject = match &record.subject {
        ExactWatchSubject::Cord { cord } => serde_json::json!({
            "kind": "cord",
            "cord": cord,
        }),
        ExactWatchSubject::NodePort {
            node,
            port,
            direction,
        } => serde_json::json!({
            "kind": "node-port",
            "node": node,
            "port": port,
            "direction": direction,
        }),
    };
    let material = match &record.material {
        ExactWatchMaterial::Preview(bytes) => browser_watch_preview(record, bytes),
        ExactWatchMaterial::Redacted => serde_json::json!({"kind": "redacted"}),
        ExactWatchMaterial::Absent => serde_json::json!({"kind": "absent"}),
    };
    serde_json::json!({
        "cursor": record.cursor,
        "source_sequence": record.source_sequence,
        "tick": record.tick,
        "watch_id": record.watch_id,
        "subject": subject,
        "producing_host": record.producing_host,
        "host_observation": record.host_observation,
        "time_basis": record.time_basis,
        "clock_uncertainty_ticks": record.clock_uncertainty_ticks,
        "value_timestamps": record.value_timestamps.iter().map(|timestamp| serde_json::json!({
            "clock_domain": timestamp.clock_domain,
            "tick": timestamp.tick,
            "uncertainty_ticks": timestamp.uncertainty_ticks,
        })).collect::<Vec<_>>(),
        "value_handle": record.value_handle,
        "accounted_bytes": record.accounted_bytes,
        "representation": {
            "id": record.representation_id,
            "schema_version": record.representation_schema_version,
            "semantic_hash": record.representation_semantic_hash.to_string(),
        },
        "sensitivity": record.sensitivity.as_str(),
        "value_identity": record.value_identity.map(|value| value.to_string()),
        "provenance": record.provenance.map(|value| value.to_string()),
        "content_hash": record.content_hash.map(|value| value.to_string()),
        "original_bytes": record.original_bytes,
        "truncated": record.truncated,
        "gap_before": record.gap_before,
        "material": material,
    })
}

fn browser_watch_preview(record: &ExactWatchObservation, bytes: &[u8]) -> serde_json::Value {
    let mut projection = serde_json::json!({
        "kind": "preview",
        "bytes": bytes,
        "renderer": {
            "status": "missing",
            "id": serde_json::Value::Null,
            "derivation": serde_json::Value::Null,
        },
    });
    let Some(object) = projection.as_object_mut() else {
        unreachable!("Watch preview projection is an object")
    };
    match record.representation_id.as_str() {
        "std/text" => {
            object.insert(
                "renderer".to_owned(),
                serde_json::json!({
                    "status": "rendered",
                    "id": "conduit.browser/utf8-text",
                    "derivation": "identity",
                }),
            );
            object.insert(
                "text".to_owned(),
                std::str::from_utf8(bytes)
                    .ok()
                    .map_or(serde_json::Value::Null, |text| {
                        serde_json::Value::String(text.to_owned())
                    }),
            );
        }
        "std/record" if !record.truncated => {
            if let Some(fields) = browser_closed_record_fields(bytes) {
                object.insert(
                    "renderer".to_owned(),
                    serde_json::json!({
                        "status": "rendered",
                        "id": "conduit.browser/closed-record-fields",
                        "derivation": "exact-closed-record-field-bytes",
                    }),
                );
                object.insert("record".to_owned(), serde_json::json!({"fields": fields}));
            }
        }
        "conduit.media/audio-frame" => {
            object.insert(
                "renderer".to_owned(),
                serde_json::json!({
                    "status": "rendered",
                    "id": "conduit.browser/audio-frame-summary",
                    "derivation": "bounded-byte-summary",
                }),
            );
            object.insert(
                "derived".to_owned(),
                serde_json::json!({"kind": "audio", "preview_bytes": bytes.len()}),
            );
        }
        "conduit.media/video-frame" => {
            object.insert(
                "renderer".to_owned(),
                serde_json::json!({
                    "status": "rendered",
                    "id": "conduit.browser/video-frame-summary",
                    "derivation": "bounded-byte-summary",
                }),
            );
            object.insert(
                "derived".to_owned(),
                serde_json::json!({"kind": "image", "preview_bytes": bytes.len()}),
            );
        }
        _ => {}
    }
    projection
}

fn browser_closed_record_fields(bytes: &[u8]) -> Option<Vec<serde_json::Value>> {
    fn take_u16(bytes: &[u8], cursor: &mut usize) -> Option<usize> {
        let end = cursor.checked_add(2)?;
        let value = u16::from_be_bytes(bytes.get(*cursor..end)?.try_into().ok()?);
        *cursor = end;
        Some(usize::from(value))
    }
    fn take_u32(bytes: &[u8], cursor: &mut usize) -> Option<usize> {
        let end = cursor.checked_add(4)?;
        let value = u32::from_be_bytes(bytes.get(*cursor..end)?.try_into().ok()?);
        *cursor = end;
        usize::try_from(value).ok()
    }

    let mut cursor = 0;
    let field_count = take_u16(bytes, &mut cursor)?;
    let mut fields = Vec::new();
    fields.try_reserve_exact(field_count).ok()?;
    for _ in 0..field_count {
        let name_length = take_u16(bytes, &mut cursor)?;
        let name_end = cursor.checked_add(name_length)?;
        let name = std::str::from_utf8(bytes.get(cursor..name_end)?).ok()?;
        cursor = name_end;
        let value_length = take_u32(bytes, &mut cursor)?;
        let value_end = cursor.checked_add(value_length)?;
        let value = bytes.get(cursor..value_end)?;
        cursor = value_end;
        fields.push(serde_json::json!({"name": name, "bytes": value}));
    }
    (cursor == bytes.len()).then_some(fields)
}

fn browser_run_io(run: &BrowserExactRun) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    run.terminal.as_ref().map_or_else(
        || {
            run.session
                .as_ref()
                .expect("live browser run retains its exact session")
                .with_io(|io| {
                    (
                        io.output().to_vec(),
                        io.error().to_vec(),
                        io.display().to_vec(),
                    )
                })
        },
        |terminal| {
            (
                terminal.output.clone(),
                terminal.error.clone(),
                terminal.display.clone(),
            )
        },
    )
}

fn browser_session_view(
    session: &BrowserPatchbaySession,
) -> Result<conduit_patchbay::PatchbayViewModel, conduit_patchbay::ProtocolError> {
    let (plan, run, high_water, evidence) = match session.run.as_ref() {
        Some(active) => (
            Some(active.plan.clone()),
            Some(patchbay_run_snapshot(active)),
            Some(patchbay_high_water(browser_run_high_water(active))),
            browser_run_evidence(active).map_err(|error| conduit_patchbay::ProtocolError {
                code: error.code,
                message: error.to_string(),
                diagnostics: Vec::new(),
                disposition: conduit_patchbay::EditDisposition::Rejected,
            })?,
        ),
        None => (None, None, None, Vec::new()),
    };
    authoritative_patchbay_view(&session.workspace, plan, run, high_water, &evidence)
}

fn finalize_browser_run_if_terminal(run: &mut BrowserExactRun) -> Result<(), RuntimeError> {
    if run.terminal.is_none() && matches!(browser_run_state(run), ExactRunState::Terminal(_)) {
        let mut session = run
            .session
            .take()
            .expect("terminal browser run retains its exact session before finalization");
        let (output, error, display) = session.with_io(|io| {
            (
                io.output().to_vec(),
                io.error().to_vec(),
                io.display().to_vec(),
            )
        });
        let terminal = BrowserExactRunTerminal {
            state: session.state(),
            high_water: session.high_water(),
            watches: run
                .watch_admissions
                .iter()
                .map(|watch| {
                    let authority = browser_watch_use_authority(
                        run,
                        &watch.operator,
                        &watch.id,
                        ExactWatchOperation::Read,
                    )?;
                    session
                        .read_watch(&watch.id, 0, watch.maximum_history, &authority)
                        .map(|batch| (watch.id.clone(), batch))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?,
            output,
            error,
            display,
        };
        session.finalize().map_err(|state| {
            RuntimeError::new(
                "CND-RUN-009",
                format!("terminal browser run could not finalize: {state:?}"),
            )
        })?;
        run.terminal = Some(terminal);
    }
    Ok(())
}

fn browser_watch_admissions(
    topology: &conduit_runtime::ExactTopologyView,
) -> Vec<WatchAdmissionDocument> {
    topology
        .cords
        .iter()
        .map(|cord| WatchAdmissionDocument {
            id: format!("watch/{}", cord.id),
            subject_kind: "cord".to_owned(),
            operator: BROWSER_WATCH_OPERATOR.to_owned(),
            control_grant_hash: browser_evidence_hash(
                b"watch-control-grant",
                &[BROWSER_WATCH_OPERATOR.as_bytes(), cord.id.as_bytes()],
            )
            .to_string(),
            lease: format!("lease/watch/{}", cord.id),
            cord: Some(cord.id.clone()),
            node: None,
            port: None,
            direction: None,
            representation: PinDocument {
                id: cord.from_port.value_type.contract_id.to_string(),
                schema_version: cord.from_port.value_type.schema_version,
                semantic_hash: cord.from_port.value_type.semantic_hash.to_string(),
            },
            maximum_preview_bytes: cord
                .max_value_bytes
                .min(MAXIMUM_BROWSER_WATCH_PREVIEW_BYTES),
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: "latest".to_owned(),
            sensitivity_ceiling: "public".to_owned(),
            reveal_action: None,
            reveal_grant_hash: None,
        })
        .collect()
}

fn start_browser_exact_run(
    source: &str,
    source_revision: u64,
    session_id: &str,
) -> Result<BrowserExactRun, RuntimeError> {
    let panel = conduit_panel::parse(source)
        .map_err(|error| RuntimeError::new("CND-SRC-001", error.to_string()))?;
    let run_id = browser_exact_run_id(session_id, source_revision);
    let registry = browser_registry();
    let topology = registry
        .resolve(&panel)
        .and_then(|resolved| resolved.exact_topology())
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    let installed = InstalledProfile::observe_registry_on_host(
        source,
        &registry,
        &browser_host_observation(),
        &[],
    )?
    .with_implementation_preference(vec![
        conduit_audio::transform_implementations::MediaImplementation::BrowserWasmLinked
            .id()
            .to_owned(),
    ])?
    .with_evidence_provider_observation(browser_evidence_provider_observation())?
    .with_watch_admissions(browser_watch_admissions(&topology))?;
    let document = compile_source(source, &installed.input)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let arena = bumpalo::Bump::new();
    let plan = document
        .as_plan(&arena)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let watch_admissions = plan
        .watch_admissions
        .iter()
        .map(|watch| {
            let cord = plan
                .cords
                .iter()
                .find(|cord| match watch.subject {
                    conduit_core::WatchSubject::Cord(id) => cord.id == id,
                    conduit_core::WatchSubject::NodePort {
                        node,
                        port,
                        direction,
                    } => {
                        let endpoint = if direction == conduit_core::Direction::Output {
                            cord.from
                        } else {
                            cord.to
                        };
                        endpoint.node == node && endpoint.port == port
                    }
                })
                .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch cord is absent"))?;
            let node = plan
                .nodes
                .iter()
                .find(|node| node.instance == cord.from.node)
                .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch producer is absent"))?;
            let host = plan
                .host_observations
                .iter()
                .find(|host| host.id == node.host_observation)
                .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch host is absent"))?;
            Ok(BrowserWatchAdmission {
                id: watch.id.to_string(),
                maximum_history: u32::from(watch.maximum_history),
                operator: watch.operator.to_string(),
                control_grant_hash: watch.control_grant_hash,
                control_grant_active: true,
                lease: watch.lease.to_string(),
                lease_available: true,
                reveal_grant_hash: watch.reveal_grant_hash,
                // The browser host does not currently have a separate reveal
                // grant observer. Non-public Watches therefore fail closed.
                reveal_grant_active: false,
                time_basis: host.time_basis.to_string(),
                valid_until_tick: host.valid_until_tick,
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    if plan.budget.memory_bytes > MAXIMUM_BROWSER_ACTIVE_RUN_MEMORY_BYTES {
        return Err(RuntimeError::new(
            "CND-RUN-009",
            "browser active-run capacity is smaller than the exact plan memory budget",
        ));
    }
    let bindings = installed.bindings(&plan)?;
    let grant_observations = installed.grant_observations(&plan)?;
    let use_observations = hosted_service_use_observations(&grant_observations);
    let mut plan_snapshot = conduit_patchbay::PlanSnapshot::from_exact_plan(&plan);
    let evidence_bytes = plan.budget.evidence_bytes;
    let node_count = plan.nodes.len();
    let cord_count = plan.cords.len();
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    pin_plan_semantic_promises(&mut plan_snapshot, &resolved.view());
    let context = ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: source_revision,
        run_id: conduit_core::Id(&run_id),
        grant_observations: &grant_observations,
        validation: conduit_core::PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 0,
            max_tick: u64::MAX,
            max_consecutive_yields: 8,
            max_events: if plan.nodes.len() > 4 { 256 } else { 128 },
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    };
    let evidence_binding = exact_evidence_provider_binding(&plan)?;
    let evidence_authority = Rc::new(RefCell::new(Some(browser_evidence_authority(
        &evidence_binding,
        &run_id,
        source_revision,
    ))));
    let evidence = Rc::new(RefCell::new(BrowserEvidenceStore::new(evidence_bytes)?));
    let evidence_provider = BrowserEvidenceProvider {
        binding: evidence_binding.clone(),
        store: Rc::clone(&evidence),
        authority: Rc::clone(&evidence_authority),
    };
    let session = resolved.start_exact_session_with_evidence_provider(
        &plan,
        &bindings,
        context,
        &browser_run_registry()?,
        ExactRunIo::for_plan(&plan)?,
        Box::new(evidence_provider),
    )?;
    let mut run = BrowserExactRun {
        plan: plan_snapshot,
        run_id,
        source_revision,
        node_count,
        cord_count,
        session: Some(session),
        use_observations,
        watch_admissions,
        evidence,
        terminal: None,
    };
    drain_browser_exact_evidence(&mut run)?;
    Ok(run)
}

/// Opens one finite, revisioned Patchbay authoring session.
#[wasm_bindgen]
pub fn patchbay_open_session(document_id: String, source: String) -> String {
    if document_id.is_empty() || document_id.len() > MAXIMUM_PATCHBAY_SESSION_ID_BYTES {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "Patchbay session identity exceeds its finite bound",
            "diagnostics": [],
            "disposition": "rejected",
        })
        .to_string();
    }
    let workspace = match conduit_patchbay::Workspace::new_with_history(
        document_id.clone(),
        source,
        conduit_patchbay::DEFAULT_WORKSPACE_HISTORY_LIMIT,
    ) {
        Ok(workspace) => workspace,
        Err(error) => return patchbay_error(error),
    };
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        if !sessions.contains_key(&document_id) && sessions.len() >= MAXIMUM_PATCHBAY_SESSIONS {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-006",
                "diagnostic": "finite Patchbay session capacity exhausted",
                "diagnostics": [],
                "disposition": "rejected",
            })
            .to_string();
        }
        if sessions
            .get(&document_id)
            .and_then(|session| session.run.as_ref())
            .is_some_and(|run| !matches!(browser_run_state(run), ExactRunState::Terminal(_)))
        {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "cannot replace a Patchbay workspace while its exact run is live",
                "diagnostics": [],
                "disposition": "rejected",
            })
            .to_string();
        }
        let session = BrowserPatchbaySession {
            workspace,
            run: None,
        };
        let view = match browser_session_view(&session) {
            Ok(view) => view,
            Err(error) => return patchbay_error(error),
        };
        sessions.insert(document_id.clone(), session);
        serde_json::json!({
            "ok": true,
            "session_id": document_id,
            "view": view,
        })
        .to_string()
    })
}

/// Inspects one unloaded definition/source without opening a session or
/// consulting the browser registry. The returned shape has no plan, run,
/// resource, authority, or evidence fields.
#[wasm_bindgen]
pub fn patchbay_inspect_at_rest(document_id: String, source: String) -> String {
    if document_id.is_empty() || document_id.len() > MAXIMUM_PATCHBAY_SESSION_ID_BYTES {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "Patchbay inspection identity exceeds its finite bound",
            "diagnostics": [],
            "disposition": "rejected",
        })
        .to_string();
    }
    match conduit_patchbay::inspect_at_rest(document_id, source) {
        Ok(inspection) => serde_json::json!({
            "ok": true,
            "inspection": inspection,
        })
        .to_string(),
        Err(error) => patchbay_error(error),
    }
}

/// Returns the current authoritative Rust projection for a Patchbay session.
#[wasm_bindgen]
pub fn patchbay_session_view(session_id: String) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let Some(session) = sessions.get(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
                "diagnostics": [],
                "disposition": "rejected",
            })
            .to_string();
        };
        match browser_session_view(session) {
            Ok(view) => serde_json::json!({"ok": true, "view": view}).to_string(),
            Err(error) => patchbay_rejection(error, &session.workspace),
        }
    })
}

/// Returns one bounded authoritative recovery snapshot for an exact run.
/// Callers use this after an evidence or Watch cursor gap; the snapshot names
/// the retained cursor windows but does not replay unbounded history.
#[wasm_bindgen]
pub fn patchbay_snapshot_exact_run(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let Some(session) = sessions.get(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_ref() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        browser_run_result(session)
    })
}

fn browser_run_state_name(state: ExactRunState) -> &'static str {
    match state {
        ExactRunState::Active => "active",
        ExactRunState::Waiting => "waiting",
        ExactRunState::Quiescing => "quiescing",
        ExactRunState::Aborting => "aborting",
        ExactRunState::Terminal(TerminalClass::Succeeded) => "succeeded",
        ExactRunState::Terminal(TerminalClass::Cancelled) => "cancelled",
        ExactRunState::Terminal(TerminalClass::Disconnected) => "disconnected",
        ExactRunState::Terminal(TerminalClass::Failed) => "failed",
    }
}

fn validate_browser_run_identity(
    run: &BrowserExactRun,
    run_id: &str,
    source_revision: u64,
    plan_identity: &str,
) -> Result<(), String> {
    if run.run_id == run_id
        && run.source_revision == source_revision
        && run.plan.identity == plan_identity
    {
        Ok(())
    } else {
        Err(serde_json::json!({
            "ok": false,
            "code": "CND-PBY-016",
            "diagnostic": "stale exact-run identity",
            "expected": {
                "run_id": run.run_id,
                "source_revision": run.source_revision,
                "plan_identity": run.plan.identity,
            },
        })
        .to_string())
    }
}

fn browser_run_result(session: &BrowserPatchbaySession) -> String {
    let Some(run) = session.run.as_ref() else {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-015",
            "diagnostic": "Patchbay session has no active exact run",
        })
        .to_string();
    };
    let state = browser_run_state(run);
    let (output, error, display) = browser_run_io(run);
    let evidence = match browser_run_evidence(run) {
        Ok(evidence) => evidence,
        Err(error) => {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
    };
    let terminal = match state {
        ExactRunState::Terminal(class) => Some(terminal_name(class)),
        ExactRunState::Active
        | ExactRunState::Waiting
        | ExactRunState::Quiescing
        | ExactRunState::Aborting => None,
    };
    let completed_nodes = usize::from(matches!(
        state,
        ExactRunState::Terminal(TerminalClass::Succeeded)
    )) * run.node_count;
    let cords_conducted = usize::from(matches!(
        state,
        ExactRunState::Terminal(TerminalClass::Succeeded)
    )) * run.cord_count;
    let next_timer_deadline = run
        .session
        .as_ref()
        .and_then(ExactHostedRunSession::next_timer_deadline);
    let (earliest_event_cursor, next_event_cursor, evidence_store) = {
        let store = run.evidence.borrow();
        (store.earliest_cursor, store.next_cursor, store.usage())
    };
    let value_storage = run
        .session
        .as_ref()
        .and_then(ExactHostedRunSession::value_storage_usage)
        .map(|usage| {
            serde_json::json!({
                "resident_slots": usage.resident_slots,
                "resident_bytes": usage.resident_bytes,
                "high_water_slots": usage.high_water_slots,
                "high_water_bytes": usage.high_water_bytes,
                "maximum_slots": usage.maximum_slots,
                "maximum_bytes": usage.maximum_bytes,
            })
        });
    match browser_session_view(session) {
        Ok(view) => serde_json::json!({
            "ok": true,
            "run_id": run.run_id,
            "source_revision": run.source_revision,
            "plan_identity": run.plan.identity,
            "source_semantic_hash": run.plan.source_semantic_hash,
            "state": browser_run_state_name(state),
            "terminal": terminal,
            "completed_nodes": completed_nodes,
            "cords_conducted": cords_conducted,
            "next_timer_deadline": next_timer_deadline,
            "earliest_event_cursor": earliest_event_cursor,
            "next_event_cursor": next_event_cursor,
            "evidence_store": evidence_store,
            "value_storage": value_storage,
            "stdout": String::from_utf8_lossy(&output),
            "stderr": String::from_utf8_lossy(&error),
            "display": String::from_utf8_lossy(&display),
            "evidence": evidence,
            "view": view,
        })
        .to_string(),
        Err(error) => patchbay_rejection(error, &session.workspace),
    }
}

/// Explicitly starts one browser-worker exact run from the current source
/// revision. This is the only operation that may create a new run epoch;
/// authoring and checking remain non-actuating.
#[wasm_bindgen]
pub fn patchbay_start_exact_run(session_id: String) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        if session
            .run
            .as_ref()
            .is_some_and(|run| !matches!(browser_run_state(run), ExactRunState::Terminal(_)))
        {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "Patchbay session already owns a live exact run",
            })
            .to_string();
        }
        let source = session.workspace.source().source.clone();
        let source_revision = session.workspace.source().revision;
        match start_browser_exact_run(&source, source_revision, &session_id) {
            Ok(run) => {
                session.run = Some(run);
                browser_run_result(session)
            }
            Err(error) => serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string(),
        }
    })
}

/// Runs one bounded cooperative turn of the active browser-worker exact run.
#[wasm_bindgen]
pub fn patchbay_pump_exact_run(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    quantum: u64,
) -> String {
    if quantum == 0 || quantum > MAXIMUM_BROWSER_RUN_PUMP_DECISIONS {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "browser exact-run pump quantum exceeds its fixed bound",
        })
        .to_string();
    }
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let Some(exact_session) = run.session.as_mut() else {
            return browser_run_result(session);
        };
        let result = exact_session.pump(quantum, &run.use_observations);
        if let Err(error) = result {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = drain_browser_exact_evidence(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = finalize_browser_run_if_terminal(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        browser_run_result(session)
    })
}

/// Returns one bounded, read-only delta from the worker-owned committed
/// evidence provider. Patchbay never acknowledges or releases scheduler
/// storage and therefore cannot become the authoritative evidence store.
#[wasm_bindgen]
pub fn patchbay_read_exact_evidence(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    cursor: u64,
    maximum_events: u32,
) -> String {
    if maximum_events == 0 || u64::from(maximum_events) > MAXIMUM_BROWSER_RUN_PUMP_DECISIONS {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "browser exact-evidence read exceeds its fixed event bound",
        })
        .to_string();
    }
    PATCHBAY_SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let Some(session) = sessions.get(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_ref() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        match browser_exact_evidence_delta(run, cursor, maximum_events) {
            Ok(batch) => serde_json::json!({
                "ok": true,
                "run_id": run.run_id,
                "plan_identity": run.plan.identity,
                "status": browser_evidence_cursor_status(batch.status),
                "next_cursor": batch.next_cursor,
                "records": batch.records,
            })
            .to_string(),
            Err(error) => serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string(),
        }
    })
}

/// Attaches one slot already admitted by the active exact plan. This changes
/// observation control only; source and plan identities remain pinned.
#[wasm_bindgen]
pub fn patchbay_attach_exact_watch(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    operator_id: String,
    watch_id: String,
) -> String {
    if watch_id.is_empty() || watch_id.len() > MAXIMUM_PATCHBAY_SESSION_ID_BYTES {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "Watch identity exceeds its fixed bound",
        })
        .to_string();
    }
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "Patchbay session has no exact run",
            })
            .to_string();
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let authority = match browser_watch_use_authority(
            run,
            &operator_id,
            &watch_id,
            ExactWatchOperation::Attach,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                return serde_json::json!({
                    "ok": false,
                    "code": error.code,
                    "diagnostic": error.to_string(),
                })
                .to_string();
            }
        };
        let Some(exact_session) = run.session.as_mut() else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "terminal exact run cannot attach a Watch",
            })
            .to_string();
        };
        match exact_session.attach_watch(&watch_id, &authority) {
            Ok(()) => serde_json::json!({
                "ok": true,
                "run_id": run.run_id,
                "plan_identity": run.plan.identity,
                "source_semantic_hash": run.plan.source_semantic_hash,
                "watch_id": watch_id,
                "attached": true,
                "usage": browser_watch_usage(exact_session.watch_usage()),
            })
            .to_string(),
            Err(error) => serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string(),
        }
    })
}

/// Detaches one active Watch while preserving its bounded retained window.
#[wasm_bindgen]
pub fn patchbay_detach_exact_watch(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    operator_id: String,
    watch_id: String,
) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "Patchbay session has no exact run",
            })
            .to_string();
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let authority = match browser_watch_use_authority(
            run,
            &operator_id,
            &watch_id,
            ExactWatchOperation::Detach,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                return serde_json::json!({
                    "ok": false,
                    "code": error.code,
                    "diagnostic": error.to_string(),
                })
                .to_string();
            }
        };
        let Some(exact_session) = run.session.as_mut() else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "terminal exact run has no attached Watch",
            })
            .to_string();
        };
        match exact_session.detach_watch(&watch_id, &authority) {
            Ok(()) => serde_json::json!({
                "ok": true,
                "run_id": run.run_id,
                "plan_identity": run.plan.identity,
                "source_semantic_hash": run.plan.source_semantic_hash,
                "watch_id": watch_id,
                "attached": false,
                "usage": browser_watch_usage(exact_session.watch_usage()),
            })
            .to_string(),
            Err(error) => serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string(),
        }
    })
}

/// Reads one bounded Watch delta from a live session or its final retained
/// window. Binary bytes remain bytes; only the exact `std/text`
/// representation receives a UTF-8 text projection.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)] // Flat WASM ABI keeps every exact identity explicit.
pub fn patchbay_read_exact_watch(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    operator_id: String,
    watch_id: String,
    cursor: u64,
    maximum_records: u32,
) -> String {
    if maximum_records == 0 || u64::from(maximum_records) > MAXIMUM_BROWSER_RUN_PUMP_DECISIONS {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "browser Watch read exceeds its fixed record bound",
        })
        .to_string();
    }
    PATCHBAY_SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let Some(session) = sessions.get(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_ref() else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "Patchbay session has no exact run",
            })
            .to_string();
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let authority = match browser_watch_use_authority(
            run,
            &operator_id,
            &watch_id,
            ExactWatchOperation::Read,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                return serde_json::json!({
                    "ok": false,
                    "code": error.code,
                    "diagnostic": error.to_string(),
                })
                .to_string();
            }
        };
        match browser_watch_delta(run, &watch_id, cursor, maximum_records, &authority) {
            Ok(batch) => serde_json::json!({
                "ok": true,
                "run_id": run.run_id,
                "plan_identity": run.plan.identity,
                "source_semantic_hash": run.plan.source_semantic_hash,
                "watch_id": watch_id,
                "status": browser_evidence_cursor_status(batch.status),
                "earliest_cursor": batch.earliest_cursor,
                "next_cursor": batch.next_cursor,
                "records": batch
                    .records
                    .iter()
                    .map(browser_watch_observation)
                    .collect::<Vec<_>>(),
            })
            .to_string(),
            Err(error) => serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string(),
        }
    })
}

/// Advances deterministic browser-host time to an exact pending deadline.
/// It is an explicit host wake, not a JavaScript executor or clock jump.
#[wasm_bindgen]
pub fn patchbay_advance_exact_run(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    tick: u64,
) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let Some(exact_session) = run.session.as_mut() else {
            return browser_run_result(session);
        };
        let result = exact_session.advance_to(tick, &run.use_observations);
        if let Err(error) = result {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = drain_browser_exact_evidence(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = finalize_browser_run_if_terminal(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        browser_run_result(session)
    })
}

/// Delivers one exact named host-operation wake to the browser-owned session.
/// The supplied subject is validated but never retained by the bridge; only an
/// already registered exact wait can become runnable.
#[wasm_bindgen]
pub fn patchbay_notify_host_operation(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    subject: String,
) -> String {
    if subject.len() > MAXIMUM_PATCHBAY_SESSION_ID_BYTES {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "browser host-operation subject exceeds its fixed byte bound",
        })
        .to_string();
    }
    let subject = match conduit_core::Id::new(&subject) {
        Ok(subject) => subject,
        Err(error) => {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-012",
                "diagnostic": format!("invalid browser host-operation subject: {error}"),
            })
            .to_string();
        }
    };
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let Some(exact_session) = run.session.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) = exact_session.notify_host_operation(subject, &run.use_observations) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = drain_browser_exact_evidence(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = finalize_browser_run_if_terminal(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        browser_run_result(session)
    })
}

/// Requests the exact plan-visible stop disposition for the active browser
/// run. `drain` and `abort` stay distinct through the shared runtime session.
#[wasm_bindgen]
pub fn patchbay_cancel_exact_run(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
    disposition: String,
) -> String {
    let stop = match disposition.as_str() {
        "drain" => conduit_core::StopPolicy::Drain,
        "abort" => conduit_core::StopPolicy::Abort,
        _ => {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-012",
                "diagnostic": "browser exact-run cancellation must be `drain` or `abort`",
            })
            .to_string();
        }
    };
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        let Some(exact_session) = run.session.as_mut() else {
            return browser_run_result(session);
        };
        if let Err(error) = exact_session.cancel(stop) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = drain_browser_exact_evidence(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        if let Err(error) = finalize_browser_run_if_terminal(run) {
            return serde_json::json!({
                "ok": false,
                "code": error.code,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
        browser_run_result(session)
    })
}

/// Releases one terminal exact run while retaining its authoring workspace.
/// Live, waiting, quiescing, and aborting runs must first reach a terminal
/// state through the production executor.
#[wasm_bindgen]
pub fn patchbay_dispose_exact_run(
    session_id: String,
    run_id: String,
    source_revision: u64,
    plan_identity: String,
) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
            })
            .to_string();
        };
        let Some(run) = session.run.as_ref() else {
            return browser_run_result(session);
        };
        if let Err(error) =
            validate_browser_run_identity(run, &run_id, source_revision, &plan_identity)
        {
            return error;
        }
        if !matches!(browser_run_state(run), ExactRunState::Terminal(_)) {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-015",
                "diagnostic": "cannot dispose a nonterminal exact run",
            })
            .to_string();
        }
        session.run = None;
        match browser_session_view(session) {
            Ok(view) => serde_json::json!({
                "ok": true,
                "disposed_run_id": run_id,
                "view": view,
            })
            .to_string(),
            Err(error) => patchbay_rejection(error, &session.workspace),
        }
    })
}

/// Applies one typed candidate transaction against persistent session
/// revisions. Candidate source is resolved and exactly planned before commit.
#[wasm_bindgen]
pub fn patchbay_apply_transaction(session_id: String, request_json: String) -> String {
    if request_json.len() > MAXIMUM_PATCHBAY_REQUEST_BYTES {
        return serde_json::json!({
            "ok": false,
            "code": "CND-PBY-006",
            "diagnostic": "typed Patchbay transaction exceeds its finite byte budget",
            "diagnostics": [],
            "candidate_revision": null,
            "compatibility": {
                "compatible": false,
                "code": "CND-PBY-006",
                "producer_type": null,
                "consumer_type": null,
                "candidate_plan_identity": null,
                "plan_disposition": "rejected",
            },
            "disposition": "rejected",
        })
        .to_string();
    }
    let request = match serde_json::from_str::<conduit_patchbay::EditRequest>(&request_json) {
        Ok(request) => request,
        Err(error) => {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-012",
                "diagnostic": format!("invalid typed Patchbay transaction: {error}"),
                "diagnostics": [],
                "candidate_revision": null,
                "compatibility": {
                    "compatible": false,
                    "code": "CND-PBY-012",
                    "producer_type": null,
                    "consumer_type": null,
                    "candidate_plan_identity": null,
                    "plan_disposition": "rejected",
                },
                "disposition": "rejected",
            })
            .to_string();
        }
    };
    PATCHBAY_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
                "diagnostics": [],
                "disposition": "rejected",
            })
            .to_string();
        };
        let registry = browser_registry();
        let result = session.workspace.apply_validated(
            request,
            |contract_id| availability_projection(&registry, contract_id),
            validate_patchbay_candidate,
        );
        match result {
            Ok(result) => match browser_session_view(session) {
                Ok(view) => serde_json::json!({
                    "ok": true,
                    "result": result,
                    "view": view,
                    "history_retained": session.workspace.history().len(),
                })
                .to_string(),
                Err(error) => patchbay_rejection(error, &session.workspace),
            },
            Err(error) => patchbay_rejection(error, &session.workspace),
        }
    })
}

fn availability_projection(
    registry: &conduit_runtime::Registry,
    contract_id: &str,
) -> conduit_patchbay::NodeAvailabilityProjection {
    let availability = registry.node_availability(contract_id);
    conduit_patchbay::NodeAvailabilityProjection {
        contract_id: availability.contract_id,
        availability_state: availability.state.as_str().to_owned(),
        reason_code: availability.reason_code,
        implementation_id: availability.implementation_id,
        host_id: availability.host_id,
        rejection_reasons: availability.rejection_reasons,
    }
}

fn validate_patchbay_candidate(
    source: &str,
) -> Result<conduit_patchbay::CompatibilityProof, conduit_patchbay::ProtocolError> {
    let panel = conduit_panel::parse(source).map_err(|error| conduit_patchbay::ProtocolError {
        code: "CND-PBY-004",
        message: "candidate source failed parser validation".to_owned(),
        diagnostics: vec![error.to_string()],
        disposition: conduit_patchbay::EditDisposition::Rejected,
    })?;
    browser_registry()
        .resolve(&panel)
        .map_err(|error| conduit_patchbay::ProtocolError {
            code: "CND-PBY-010",
            message: "candidate source failed resolver compatibility validation".to_owned(),
            diagnostics: vec![format!("{}: {}", error.code, error.message)],
            disposition: conduit_patchbay::EditDisposition::Rejected,
        })?;
    let candidate_plan_identity = exact_plan_snapshot(source).map(|plan| plan.identity);
    let plan_disposition = if candidate_plan_identity.is_some() {
        "candidate-only"
    } else {
        "not-applicable"
    };
    Ok(conduit_patchbay::CompatibilityProof {
        compatible: true,
        code: if candidate_plan_identity.is_some() {
            "CND-PBY-EXACT-PLAN".to_owned()
        } else {
            "CND-PBY-RESOLVED".to_owned()
        },
        producer_type: None,
        consumer_type: None,
        candidate_plan_identity,
        plan_disposition: plan_disposition.to_owned(),
    })
}

fn exact_plan_snapshot(source: &str) -> Option<conduit_patchbay::PlanSnapshot> {
    let registry = browser_registry();
    let panel = conduit_panel::parse(source).ok()?;
    let resolved = registry.resolve(&panel).ok()?;
    let topology = resolved.exact_topology().ok()?;
    let installed = InstalledProfile::observe_registry_on_host(
        source,
        &registry,
        &browser_host_observation(),
        &[],
    )
    .ok()?
    .with_implementation_preference(vec![
        conduit_audio::transform_implementations::MediaImplementation::BrowserWasmLinked
            .id()
            .to_owned(),
    ])
    .ok()?
    .with_evidence_provider_observation(browser_evidence_provider_observation())
    .ok()?
    .with_watch_admissions(browser_watch_admissions(&topology))
    .ok()?;
    let document = compile_source(source, &installed.input).ok()?;
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).ok()?;
    let mut snapshot = conduit_patchbay::PlanSnapshot::from_exact_plan(&plan);
    pin_plan_semantic_promises(&mut snapshot, &resolved.view());
    Some(snapshot)
}

fn pin_plan_semantic_promises(
    plan: &mut conduit_patchbay::PlanSnapshot,
    resolved: &conduit_runtime::ResolvedPanelView,
) {
    for binding in &mut plan.bindings {
        let Some(node) = resolved.nodes.iter().find(|node| {
            node.id == binding.instance
                || node.id.strip_prefix("root/") == Some(binding.instance.as_str())
                || binding.instance.strip_prefix("root/") == Some(node.id.as_str())
        }) else {
            continue;
        };
        binding.inherited_inputs = node
            .inputs
            .iter()
            .map(|port| project_authored_port(&binding.instance, port, "input"))
            .collect();
        binding.inherited_outputs = node
            .outputs
            .iter()
            .map(|port| project_authored_port(&binding.instance, port, "output"))
            .collect();
    }
}

fn planned_realization_projection(
    plan: &conduit_patchbay::PlanSnapshot,
    logical_nodes: &[conduit_patchbay::PatchbayNodeProjection],
    selection: &str,
    active_plan_identity: Option<String>,
    candidate_plan_identity: Option<String>,
    current_source_semantic_hash: Option<&str>,
) -> conduit_patchbay::PatchbayPlannedRealizationProjection {
    let current_source_matches =
        current_source_semantic_hash == Some(plan.source_semantic_hash.as_str());
    let nodes =
        plan.bindings
            .iter()
            .map(|binding| {
                let mut inputs = binding.inherited_inputs.clone();
                let mut outputs = binding.inherited_outputs.clone();
                for port in &mut inputs {
                    port.connected = plan
                        .cords
                        .iter()
                        .any(|cord| cord.to_node == binding.instance && cord.to_port == port.id);
                }
                for port in &mut outputs {
                    port.connected = plan.cords.iter().any(|cord| {
                        cord.from_node == binding.instance && cord.from_port == port.id
                    });
                }
                conduit_patchbay::PatchbayPlannedNodeProjection {
                    instance: binding.instance.clone(),
                    logical_origin: binding.logical_origin.clone(),
                    composite_provenance: binding.composite_provenance.clone(),
                    source_origin_range: current_source_matches
                        .then(|| {
                            logical_nodes
                                .iter()
                                .find(|node| node.id == binding.logical_origin)
                                .and_then(|node| node.source_range.clone())
                        })
                        .flatten(),
                    inputs,
                    outputs,
                    binding: binding.clone(),
                }
            })
            .collect();
    let mut notice = match (
        selection,
        current_source_matches,
        candidate_plan_identity.as_deref(),
    ) {
        ("active-run", false, Some(candidate)) if candidate != plan.identity => format!(
            "Expanded is showing active run plan {}; candidate plan {} remains separate.",
            plan.identity, candidate
        ),
        ("active-run", false, _) => format!(
            "Expanded is showing active run plan {}; the current source is a different semantic revision.",
            plan.identity
        ),
        ("active-run", true, _) => {
            format!("Expanded is showing active run plan {}.", plan.identity)
        }
        _ => format!(
            "Expanded is showing read-only candidate plan {}; changes require resolution into a new plan.",
            plan.identity
        ),
    };
    if plan.bindings.iter().any(|binding| {
        binding
            .host_observation_status
            .starts_with("stale-replan-required")
    }) {
        notice.push_str(
            " A pinned host observation is stale; its recorded fact remains visible and replan is required.",
        );
    }
    conduit_patchbay::PatchbayPlannedRealizationProjection {
        plan_identity: plan.identity.clone(),
        source_semantic_hash: plan.source_semantic_hash.clone(),
        selection: selection.to_owned(),
        current_source_matches,
        active_plan_identity,
        candidate_plan_identity,
        notice,
        nodes,
        cords: plan.cords.clone(),
        composites: plan.composites.clone(),
    }
}

fn patchbay_cycle_participants(
    cords: &[conduit_patchbay::PatchbayCordProjection],
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut cycle_nodes = BTreeSet::new();
    let mut cycle_cords = BTreeSet::new();
    for candidate in cords {
        let (Some(from), Some(to)) = (&candidate.from_node, &candidate.to_node) else {
            continue;
        };
        let mut visited = BTreeSet::new();
        let mut pending = vec![to.clone()];
        let mut returns_to_source = to == from;
        while !returns_to_source {
            let Some(node) = pending.pop() else {
                break;
            };
            if !visited.insert(node.clone()) {
                continue;
            }
            for cord in cords.iter().filter(|cord| {
                cord.from_node
                    .as_deref()
                    .is_some_and(|source| source == node)
            }) {
                let Some(next) = cord.to_node.as_ref() else {
                    continue;
                };
                if next == from {
                    returns_to_source = true;
                    break;
                }
                pending.push(next.clone());
            }
        }
        if returns_to_source {
            cycle_cords.insert(candidate.id.clone());
            cycle_nodes.insert(from.clone());
            cycle_nodes.insert(to.clone());
        }
    }
    (cycle_nodes, cycle_cords)
}

fn authoritative_patchbay_view(
    workspace: &conduit_patchbay::Workspace,
    exact_plan: Option<conduit_patchbay::PlanSnapshot>,
    run: Option<conduit_patchbay::RunSnapshot>,
    high_water: Option<conduit_patchbay::PatchbayHighWaterProjection>,
    evidence: &[serde_json::Value],
) -> Result<conduit_patchbay::PatchbayViewModel, conduit_patchbay::ProtocolError> {
    let document = conduit_panel::parse_document(&workspace.source().source);
    let recovered = conduit_panel::recover_document(&workspace.source().source);
    let registry = browser_registry();
    let semantic = workspace.semantic_with_lookup(|id| availability_projection(&registry, id));
    let bounds = conduit_patchbay::PatchbayProjectionBounds::default();
    let source_text = workspace.source().source.as_str();
    let source_revision = workspace.source().revision;
    let mut diagnostics = Vec::new();
    let mut diagnostic_anchors = Vec::new();
    let mut logical_nodes = Vec::new();
    let mut cords = Vec::new();

    if let Some(panel) = document.ast.as_ref() {
        for source_node in &panel.nodes {
            let node_range = source_range_for_span(
                source_text,
                source_node.source_span,
                source_revision,
                "authored",
            );
            let contract = registry.authored_node_view(panel, &source_node.kind);
            let mut diagnostic_ids = Vec::new();
            let duplicate = panel
                .nodes
                .iter()
                .filter(|candidate| candidate.id == source_node.id)
                .count()
                > 1;
            let validity = if duplicate {
                let id = add_patchbay_diagnostic(
                    &mut diagnostics,
                    source_revision,
                    "CND-ID-002",
                    "error",
                    "invalid",
                    format!("duplicate node id `{}`", source_node.id),
                    "Each authored node identity must be unique.",
                    node_range.clone(),
                    vec![("node", source_node.id.as_str())],
                );
                diagnostic_ids.push(id);
                "invalid"
            } else if contract.is_none() {
                let id = add_patchbay_diagnostic(
                    &mut diagnostics,
                    source_revision,
                    "CND-IMP-001",
                    "error",
                    "unresolved",
                    format!("unknown or unresolved contract `{}`", source_node.kind),
                    "No ports, provider, placement, or plan are inferred for an unresolved contract.",
                    node_range.clone(),
                    vec![("node", source_node.id.as_str())],
                );
                diagnostic_ids.push(id);
                "unresolved"
            } else {
                "valid"
            };
            let (inputs, outputs) = contract.as_ref().map_or_else(
                || (Vec::new(), Vec::new()),
                |contract| {
                    (
                        contract
                            .inputs
                            .iter()
                            .map(|port| project_authored_port(&source_node.id, port, "input"))
                            .collect(),
                        contract
                            .outputs
                            .iter()
                            .map(|port| project_authored_port(&source_node.id, port, "output"))
                            .collect(),
                    )
                },
            );
            let config = source_node
                .config
                .iter()
                .map(|entry| {
                    let mut projection = project_config_value(&entry.value, source_revision);
                    projection.source_range = source_range_for_span(
                        source_text,
                        entry.source_span,
                        source_revision,
                        "authored-config",
                    );
                    (entry.key.clone(), projection)
                })
                .collect();
            logical_nodes.push(conduit_patchbay::PatchbayNodeProjection {
                id: source_node.id.clone(),
                semantic_id: format!("root/{}", source_node.id),
                contract_id: Some(source_node.kind.clone()),
                contract_identity: contract
                    .as_ref()
                    .and_then(|contract| contract.contract_identity.clone()),
                semantic_effects: Vec::new(),
                source_range: node_range,
                inputs,
                outputs,
                config,
                // Semantic topology deliberately carries no provider, host,
                // device, artifact, or live availability observation.
                availability: None,
                validity: validity.to_owned(),
                diagnostic_ids,
                placement: None,
                activity: None,
            });
        }
        for source_cord in &panel.cords {
            let source_range = source_range_for_span(
                source_text,
                source_cord.source_span,
                source_revision,
                "authored",
            );
            let endpoint_ranges = cord_endpoint_member_offsets(source_text, source_cord);
            let from_port_range = endpoint_ranges.and_then(|(from, _)| {
                source_range_from_offsets(source_text, from, source_revision, "authored-endpoint")
            });
            let to_port_range = endpoint_ranges.and_then(|(_, to)| {
                source_range_from_offsets(source_text, to, source_revision, "authored-endpoint")
            });
            let to_diagnostic_range = endpoint_ranges.and_then(|(_, to)| {
                endpoint_source_range(
                    source_text,
                    to,
                    &source_cord.to,
                    source_revision,
                    "authored-diagnostic-endpoint",
                )
            });
            let assessment = registry.assess_authored_cord(panel, source_cord);
            let mut diagnostic_ids = Vec::new();
            if assessment.state != "valid" {
                let invalid_field = (assessment.state == "invalid-bounds")
                    .then(|| invalid_cord_field(source_text, source_cord, source_revision))
                    .flatten();
                let config_target = invalid_field
                    .as_ref()
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default();
                let mut targets = vec![
                    ("cord", source_cord.id.as_str()),
                    ("node", source_cord.from.node.as_str()),
                    ("port", source_cord.from.port.as_str()),
                    ("node", source_cord.to.node.as_str()),
                    ("port", source_cord.to.port.as_str()),
                ];
                if !config_target.is_empty() {
                    targets.push(("config", config_target.as_str()));
                }
                let id = add_patchbay_diagnostic(
                    &mut diagnostics,
                    source_revision,
                    assessment.code,
                    "error",
                    assessment.state,
                    assessment.message.clone(),
                    assessment.explanation.clone(),
                    invalid_field
                        .map(|(_, range)| range)
                        .or(to_diagnostic_range)
                        .or_else(|| to_port_range.clone())
                        .or_else(|| source_range.clone()),
                    targets,
                );
                diagnostic_ids.push(id);
            }
            let from_direction = projected_port_direction(
                &logical_nodes,
                &source_cord.from.node,
                &source_cord.from.port,
            );
            let to_direction = projected_port_direction(
                &logical_nodes,
                &source_cord.to.node,
                &source_cord.to.port,
            );
            let from_needs_anchor = from_direction.as_deref() != Some("output");
            let to_needs_anchor = to_direction.as_deref() != Some("input");
            let from_anchor = from_needs_anchor.then(|| {
                add_diagnostic_anchor(
                    &mut diagnostic_anchors,
                    source_cord,
                    "from",
                    Some(source_cord.from.node.clone()),
                    from_port_range.clone(),
                )
            });
            let to_anchor = to_needs_anchor.then(|| {
                add_diagnostic_anchor(
                    &mut diagnostic_anchors,
                    source_cord,
                    "to",
                    Some(source_cord.to.node.clone()),
                    to_port_range.clone(),
                )
            });
            cords.push(conduit_patchbay::PatchbayCordProjection {
                id: source_cord.id.clone(),
                semantic_path: format!("root/cord/{}", source_cord.id),
                owner_kind: "enclosing-panel".to_owned(),
                owner_path: "root".to_owned(),
                boundary_rule: "exported-public-ports-only".to_owned(),
                from_node: Some(source_cord.from.node.clone()),
                from_port: Some(source_cord.from.port.clone()),
                from_port_path: Some(format!(
                    "root/{}/port/{}/{}",
                    source_cord.from.node,
                    direction_path(from_direction.as_deref()),
                    source_cord.from.port
                )),
                from_port_range,
                to_node: Some(source_cord.to.node.clone()),
                to_port: Some(source_cord.to.port.clone()),
                to_port_path: Some(format!(
                    "root/{}/port/{}/{}",
                    source_cord.to.node,
                    direction_path(to_direction.as_deref()),
                    source_cord.to.port
                )),
                to_port_range,
                value_type: assessment.producer_type.clone(),
                compatibility: Some(conduit_patchbay::CompatibilityProof {
                    compatible: assessment.state == "valid",
                    code: assessment.code.to_owned(),
                    producer_type: assessment.producer_type,
                    consumer_type: assessment.consumer_type,
                    candidate_plan_identity: None,
                    plan_disposition: if assessment.state == "valid" {
                        "candidate-only"
                    } else {
                        "unavailable"
                    }
                    .to_owned(),
                }),
                capacity_items: Some(source_cord.capacity_items),
                max_value_bytes: Some(source_cord.max_value_bytes),
                max_queued_bytes: Some(source_cord.max_queued_bytes),
                low_watermark_items: Some(source_cord.low_watermark_items),
                high_watermark_items: Some(source_cord.high_watermark_items),
                pressure: Some(source_cord.pressure.to_string()),
                source_range,
                high_water_items: None,
                validity: assessment.state.to_owned(),
                diagnostic_ids,
                from_anchor,
                to_anchor,
            });
        }
    } else {
        let empty_panel = conduit_panel::parse("panel 0\n").expect("empty current panel parses");
        let recovery_panel = panel_without_recovered_cords(source_text, &recovered);
        for recovered_node in &recovered.nodes {
            let Some(id) = recovered_node.id.as_ref() else {
                continue;
            };
            let range = source_range_for_recovered_span(
                source_text,
                recovered_node.source_span,
                source_revision,
                "recovered-node",
            );
            let contract = recovered_node.kind.as_deref().and_then(|kind| {
                registry.authored_node_view(recovery_panel.as_ref().unwrap_or(&empty_panel), kind)
            });
            let duplicate = recovered
                .nodes
                .iter()
                .filter(|candidate| candidate.id.as_deref() == Some(id.as_str()))
                .count()
                > 1;
            let (validity, diagnostic_ids) = if duplicate {
                (
                    "invalid",
                    vec![add_patchbay_diagnostic(
                        &mut diagnostics,
                        source_revision,
                        "CND-ID-002",
                        "error",
                        "invalid",
                        format!("duplicate node id `{id}`"),
                        "Each authored node identity must be unique.",
                        range.clone(),
                        vec![("node", id.as_str())],
                    )],
                )
            } else if !recovered_node.complete {
                (
                    "incomplete",
                    vec![add_patchbay_diagnostic(
                        &mut diagnostics,
                        source_revision,
                        "CND-PNL-RECOVER",
                        "pending",
                        "incomplete",
                        format!("incomplete authored node `{id}`"),
                        "This provisional faceplate contains only facts recoverable from the current source.",
                        range.clone(),
                        vec![("node", id.as_str())],
                    )],
                )
            } else if contract.is_none() {
                (
                    "unresolved",
                    vec![add_patchbay_diagnostic(
                        &mut diagnostics,
                        source_revision,
                        "CND-IMP-001",
                        "error",
                        "unresolved",
                        format!(
                            "unknown or unresolved contract `{}`",
                            recovered_node.kind.as_deref().unwrap_or("")
                        ),
                        "No ports, provider, placement, or plan are inferred for an unresolved contract.",
                        range.clone(),
                        vec![("node", id.as_str())],
                    )],
                )
            } else {
                ("valid", Vec::new())
            };
            let (inputs, outputs) = contract.as_ref().map_or_else(
                || (Vec::new(), Vec::new()),
                |contract| {
                    (
                        contract
                            .inputs
                            .iter()
                            .map(|port| project_authored_port(id, port, "input"))
                            .collect(),
                        contract
                            .outputs
                            .iter()
                            .map(|port| project_authored_port(id, port, "output"))
                            .collect(),
                    )
                },
            );
            logical_nodes.push(conduit_patchbay::PatchbayNodeProjection {
                id: id.clone(),
                semantic_id: format!("source/{id}"),
                contract_id: recovered_node.kind.clone(),
                contract_identity: contract
                    .as_ref()
                    .and_then(|contract| contract.contract_identity.clone()),
                semantic_effects: Vec::new(),
                source_range: range,
                inputs,
                outputs,
                config: BTreeMap::new(),
                availability: None,
                validity: validity.to_owned(),
                diagnostic_ids,
                placement: None,
                activity: None,
            });
        }
        for recovered_cord in &recovered.cords {
            if recovered_cord.from.node.is_none() {
                continue;
            }
            let source_range = source_range_for_recovered_span(
                source_text,
                recovered_cord.source_span,
                source_revision,
                "recovered-cord",
            );
            let from_port_range = recovered_cord.from.source_span.and_then(|span| {
                source_range_for_recovered_span(
                    source_text,
                    span,
                    source_revision,
                    "recovered-endpoint",
                )
            });
            let to_port_range = recovered_cord.to.source_span.and_then(|span| {
                source_range_for_recovered_span(
                    source_text,
                    span,
                    source_revision,
                    "recovered-endpoint",
                )
            });
            let authored = recovered_cord_as_authored(source_text, recovered_cord);
            let assessment = authored
                .as_ref()
                .zip(recovery_panel.as_ref())
                .map(|(cord, panel)| registry.assess_authored_cord(panel, cord));
            let validity = assessment
                .as_ref()
                .map_or("incomplete", |assessment| assessment.state);
            let (code, severity, message, explanation) =
                assessment.as_ref().map_or_else(
                    || {
                        (
                            "CND-PNL-RECOVER",
                            "pending",
                            "incomplete authored cord".to_owned(),
                            "The recovered endpoint is a diagnostic anchor, not a synthetic semantic port."
                                .to_owned(),
                        )
                    },
                    |assessment| {
                        (
                            assessment.code,
                            "error",
                            assessment.message.clone(),
                            assessment.explanation.clone(),
                        )
                    },
                );
            let invalid_field = assessment
                .as_ref()
                .filter(|assessment| assessment.state == "invalid-bounds")
                .and(authored.as_ref())
                .and_then(|cord| invalid_cord_field(source_text, cord, source_revision));
            let config_target = invalid_field
                .as_ref()
                .map(|(id, _)| id.clone())
                .unwrap_or_default();
            let mut diagnostic_targets = vec![("cord", recovered_cord.id.as_str())];
            if !config_target.is_empty() {
                diagnostic_targets.push(("config", config_target.as_str()));
            }
            let diagnostic_ids = (validity != "valid")
                .then(|| {
                    add_patchbay_diagnostic(
                        &mut diagnostics,
                        source_revision,
                        code,
                        severity,
                        validity,
                        message,
                        explanation,
                        invalid_field
                            .map(|(_, range)| range)
                            .or_else(|| to_port_range.clone())
                            .or_else(|| source_range.clone()),
                        diagnostic_targets,
                    )
                })
                .into_iter()
                .collect::<Vec<_>>();
            let from_direction = match (&recovered_cord.from.node, &recovered_cord.from.port) {
                (Some(node), Some(port)) => projected_port_direction(&logical_nodes, node, port),
                _ => None,
            };
            let to_direction = match (&recovered_cord.to.node, &recovered_cord.to.port) {
                (Some(node), Some(port)) => projected_port_direction(&logical_nodes, node, port),
                _ => None,
            };
            let from_anchor = (from_direction.as_deref() != Some("output")).then(|| {
                add_recovered_anchor(
                    &mut diagnostic_anchors,
                    recovered_cord,
                    "from",
                    recovered_cord.from.node.clone(),
                    from_port_range.clone(),
                )
            });
            let to_anchor = (to_direction.as_deref() != Some("input")).then(|| {
                add_recovered_anchor(
                    &mut diagnostic_anchors,
                    recovered_cord,
                    "to",
                    recovered_cord.to.node.clone(),
                    to_port_range.clone(),
                )
            });
            cords.push(conduit_patchbay::PatchbayCordProjection {
                id: recovered_cord.id.clone(),
                semantic_path: format!("root/cord/{}", recovered_cord.id),
                owner_kind: "enclosing-panel".to_owned(),
                owner_path: "root".to_owned(),
                boundary_rule: "exported-public-ports-only".to_owned(),
                from_node: recovered_cord.from.node.clone(),
                from_port: recovered_cord.from.port.clone(),
                from_port_path: recovered_cord
                    .from
                    .node
                    .as_ref()
                    .zip(recovered_cord.from.port.as_ref())
                    .map(|(node, port)| {
                        format!(
                            "root/{node}/port/{}/{port}",
                            direction_path(from_direction.as_deref())
                        )
                    }),
                from_port_range,
                to_node: recovered_cord.to.node.clone(),
                to_port: recovered_cord.to.port.clone(),
                to_port_path: recovered_cord
                    .to
                    .node
                    .as_ref()
                    .zip(recovered_cord.to.port.as_ref())
                    .map(|(node, port)| {
                        format!(
                            "root/{node}/port/{}/{port}",
                            direction_path(to_direction.as_deref())
                        )
                    }),
                to_port_range,
                value_type: assessment
                    .as_ref()
                    .and_then(|assessment| assessment.producer_type.clone()),
                compatibility: assessment.as_ref().map(|assessment| {
                    conduit_patchbay::CompatibilityProof {
                        compatible: assessment.state == "valid",
                        code: assessment.code.to_owned(),
                        producer_type: assessment.producer_type.clone(),
                        consumer_type: assessment.consumer_type.clone(),
                        candidate_plan_identity: None,
                        plan_disposition: if assessment.state == "valid" {
                            "candidate-only"
                        } else {
                            "unavailable"
                        }
                        .to_owned(),
                    }
                }),
                capacity_items: authored.as_ref().map(|cord| cord.capacity_items),
                max_value_bytes: authored.as_ref().map(|cord| cord.max_value_bytes),
                max_queued_bytes: authored.as_ref().map(|cord| cord.max_queued_bytes),
                low_watermark_items: authored.as_ref().map(|cord| cord.low_watermark_items),
                high_watermark_items: authored.as_ref().map(|cord| cord.high_watermark_items),
                pressure: authored.as_ref().map(|cord| cord.pressure.to_string()),
                source_range,
                high_water_items: None,
                validity: validity.to_owned(),
                diagnostic_ids,
                from_anchor,
                to_anchor,
            });
        }
        if let Some(error) = document.diagnostics.first().filter(|_| {
            recovered.nodes.iter().any(|node| !node.complete)
                || recovered.cords.iter().any(|cord| !cord.complete)
        }) {
            add_patchbay_diagnostic(
                &mut diagnostics,
                source_revision,
                error.code,
                "pending",
                "incomplete",
                error.to_string(),
                "The current source is incomplete; only bounded Rust-recovered facts are shown.",
                parse_error_source_range(source_text, error, source_revision),
                vec![("source", workspace.source().document_id.as_str())],
            );
        }
    }

    mark_connected_ports(&mut logical_nodes, &cords);
    let resolved_result = document.ast.as_ref().map(|panel| registry.resolve(panel));
    if let Some(Err(error)) = resolved_result.as_ref()
        && error.code == "CND-CMP-001"
    {
        let (cycle_nodes, cycle_cords) = patchbay_cycle_participants(&cords);
        let id = add_patchbay_diagnostic(
            &mut diagnostics,
            source_revision,
            error.code,
            "error",
            "invalid-topology",
            error.message.clone(),
            "This dependency cycle has no explicit finite temporal boundary. Add a domain-appropriate retained-state, delay, or lifecycle boundary, or remove the cycle; scheduler order cannot define feedback.",
            cords
                .iter()
                .find(|cord| cycle_cords.contains(&cord.id))
                .and_then(|cord| cord.source_range.clone()),
            vec![("source", workspace.source().document_id.as_str())],
        );
        for node in logical_nodes
            .iter_mut()
            .filter(|node| cycle_nodes.contains(&node.id))
        {
            node.validity = "invalid-topology".to_owned();
            node.diagnostic_ids.push(id.clone());
        }
        for cord in cords
            .iter_mut()
            .filter(|cord| cycle_cords.contains(&cord.id))
        {
            cord.validity = "invalid-topology".to_owned();
            cord.diagnostic_ids.push(id.clone());
        }
    }
    let resolved = resolved_result.and_then(Result::ok);
    let resolved_view = resolved.as_ref().map(conduit_runtime::ResolvedPanel::view);
    // Candidate resolution is always kept separate from an active run's
    // pinned plan. Expanded defaults to the active plan when one exists and
    // never joins that plan with the mutable candidate topology.
    let active_plan_mismatch = run.as_ref().is_some_and(|run| {
        exact_plan
            .as_ref()
            .is_none_or(|plan| run.plan_identity != plan.identity)
    });
    let candidate_plan = resolved_view
        .as_ref()
        .and_then(|_| exact_plan_snapshot(source_text));
    let candidate_plan_identity = candidate_plan
        .as_ref()
        .map(|candidate| candidate.identity.clone());
    let (plan, matching_run, selection) = match run {
        Some(run) => match exact_plan {
            Some(plan) if run.plan_identity == plan.identity => {
                (Some(plan), Some(run), Some("active-run"))
            }
            // A caller which cannot supply the run's exact snapshot cannot
            // fall back to the candidate and pretend it describes that run.
            _ => (None, None, None),
        },
        None => (exact_plan.or(candidate_plan), None, Some("candidate")),
    };
    let mut planned_realization = plan.as_ref().zip(selection).map(|(plan, selection)| {
        planned_realization_projection(
            plan,
            &logical_nodes,
            selection,
            matching_run.as_ref().map(|run| run.plan_identity.clone()),
            candidate_plan_identity.clone(),
            semantic.source_semantic_hash.as_deref(),
        )
    });
    let planned_realization_status = if active_plan_mismatch {
        "active-plan-mismatch"
    } else if plan.is_some() {
        "exact-plan"
    } else {
        "no-exact-plan"
    }
    .to_owned();
    let mut composites = document.ast.as_ref().map_or_else(Vec::new, |panel| {
        panel
            .nodes
            .iter()
            .filter_map(|instance| {
                panel
                    .definitions
                    .iter()
                    .find(|definition| definition.id == instance.kind)
                    .map(|definition| conduit_patchbay::PatchbayCompositeProjection {
                        id: instance.id.clone(),
                        definition: definition.id.clone(),
                        members: definition
                            .nodes
                            .iter()
                            .map(|child| format!("{}.{}", instance.id, child.id))
                            .collect(),
                        internal_cords: definition
                            .cords
                            .iter()
                            .map(
                                |cord| conduit_patchbay::PatchbayOwnedInternalCordProjection {
                                    from: format!("{}.{}", cord.from.node, cord.from.port),
                                    to: format!("{}.{}", cord.to.node, cord.to.port),
                                    owner_kind: "panel-definition".to_owned(),
                                    owner_path: format!("definition/{}", definition.id),
                                },
                            )
                            .collect(),
                        exports: definition
                            .exports
                            .iter()
                            .map(|export| conduit_patchbay::PatchbayExportProjection {
                                direction: direction_name(export.direction).to_owned(),
                                id: export.id.clone(),
                                target_node: export.target.node.clone(),
                                target_port: export.target.port.clone(),
                            })
                            .collect(),
                        bindings: definition
                            .bindings
                            .iter()
                            .map(
                                |binding| conduit_patchbay::PatchbayDefinitionBindingProjection {
                                    parameter: binding.parameter.clone(),
                                    target: format!(
                                        "{}.{}",
                                        binding.target.node, binding.target.port
                                    ),
                                    owner_kind: "panel-definition".to_owned(),
                                    persistence: "source-document".to_owned(),
                                    activation: "source-candidate-requires-resolution".to_owned(),
                                },
                            )
                            .collect(),
                    })
            })
            .collect()
    });
    let mut truncated = logical_nodes.len() > bounds.maximum_nodes
        || planned_realization.as_ref().is_some_and(|realization| {
            realization.nodes.len() > bounds.maximum_nodes
                || realization.cords.len() > bounds.maximum_cords
                || realization.composites.len() > bounds.maximum_composites
        })
        || cords.len() > bounds.maximum_cords
        || composites.len() > bounds.maximum_composites
        || diagnostics.len() > bounds.maximum_diagnostics
        || evidence.len() > bounds.maximum_evidence_events;
    logical_nodes.truncate(bounds.maximum_nodes);
    cords.truncate(bounds.maximum_cords);
    composites.truncate(bounds.maximum_composites);
    diagnostics.truncate(bounds.maximum_diagnostics);
    for node in &mut logical_nodes {
        truncated |= node.inputs.len() > bounds.maximum_ports_per_node
            || node.outputs.len() > bounds.maximum_ports_per_node
            || node.config.len() > bounds.maximum_config_fields_per_node;
        node.inputs.truncate(bounds.maximum_ports_per_node);
        node.outputs.truncate(bounds.maximum_ports_per_node);
        if node.config.len() > bounds.maximum_config_fields_per_node {
            node.config = std::mem::take(&mut node.config)
                .into_iter()
                .take(bounds.maximum_config_fields_per_node)
                .collect();
        }
    }
    if let Some(realization) = &mut planned_realization {
        realization.nodes.truncate(bounds.maximum_nodes);
        realization.cords.truncate(bounds.maximum_cords);
        realization.composites.truncate(bounds.maximum_composites);
        for node in &mut realization.nodes {
            truncated |= node.inputs.len() > bounds.maximum_ports_per_node
                || node.outputs.len() > bounds.maximum_ports_per_node;
            node.inputs.truncate(bounds.maximum_ports_per_node);
            node.outputs.truncate(bounds.maximum_ports_per_node);
        }
    }
    for composite in &mut composites {
        truncated |= composite.members.len() > bounds.maximum_nodes
            || composite.exports.len() > bounds.maximum_ports_per_node;
        composite.members.truncate(bounds.maximum_nodes);
        composite.exports.truncate(bounds.maximum_ports_per_node);
    }
    let mut configuration_layers = Vec::new();
    if let Some(panel) = document.ast.as_ref() {
        for definition in &panel.definitions {
            let fields = definition
                .parameters
                .iter()
                .filter_map(|parameter| {
                    parameter.default.as_ref().map(|value| {
                        let projected = project_config_value(value, source_revision);
                        conduit_patchbay::PatchbayConfigurationFieldProjection {
                            id: parameter.id.clone(),
                            display_value: projected.display_value,
                        }
                    })
                })
                .take(bounds.maximum_config_fields_per_node)
                .collect::<Vec<_>>();
            if !fields.is_empty() {
                configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
                    id: format!("definition/{}/defaults", definition.id),
                    owner: "panel-definition".to_owned(),
                    persistence: "source-document".to_owned(),
                    revision: source_revision.to_string(),
                    sensitivity: "declared-per-field".to_owned(),
                    mutability: "definition-source-candidate".to_owned(),
                    activation: "re-resolution".to_owned(),
                    fields,
                });
            }
        }
    }
    for node in &logical_nodes {
        if !node.config.is_empty() {
            configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
                id: format!("root/{}/instance-authored", node.id),
                owner: "panel-instance".to_owned(),
                persistence: "source-document".to_owned(),
                revision: source_revision.to_string(),
                sensitivity: "declared-per-field".to_owned(),
                mutability: "source-candidate".to_owned(),
                activation: "re-resolution-or-plan-transition".to_owned(),
                fields: node
                    .config
                    .iter()
                    .map(
                        |(id, value)| conduit_patchbay::PatchbayConfigurationFieldProjection {
                            id: id.clone(),
                            display_value: value.display_value.clone(),
                        },
                    )
                    .collect(),
            });
        }
        if !node.inputs.is_empty() {
            configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
                id: format!("root/{}/live-inputs", node.id),
                owner: "upstream-semantic-cords".to_owned(),
                persistence: "run-only".to_owned(),
                revision: matching_run
                    .as_ref()
                    .map_or_else(|| "no-run".to_owned(), |run| run.run_id.clone()),
                sensitivity: "declared-per-port".to_owned(),
                mutability: "ordinary-typed-input-not-config".to_owned(),
                activation: "current-run-delivery".to_owned(),
                fields: node
                    .inputs
                    .iter()
                    .map(
                        |port| conduit_patchbay::PatchbayConfigurationFieldProjection {
                            id: port.id.clone(),
                            display_value: port.type_id.clone(),
                        },
                    )
                    .collect(),
            });
        }
    }
    if let Some(plan) = plan.as_ref() {
        for binding in &plan.bindings {
            let mut fields = vec![
                conduit_patchbay::PatchbayConfigurationFieldProjection {
                    id: "implementation".to_owned(),
                    display_value: binding.implementation_id.clone(),
                },
                conduit_patchbay::PatchbayConfigurationFieldProjection {
                    id: "host".to_owned(),
                    display_value: binding.host_id.clone(),
                },
            ];
            fields.extend(binding.resources.iter().map(|resource| {
                conduit_patchbay::PatchbayConfigurationFieldProjection {
                    id: format!("resource/{}", resource.binding_id),
                    display_value: resource.resource_kind.clone(),
                }
            }));
            configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
                id: format!("{}/resolved-binding", binding.instance),
                owner: "exact-plan".to_owned(),
                persistence: "plan-epoch".to_owned(),
                revision: plan.identity.clone(),
                sensitivity: "opaque-handles-redacted".to_owned(),
                mutability: "immutable".to_owned(),
                activation: "plan-transition".to_owned(),
                fields,
            });
        }
    }
    configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
        id: "presentation/preferences".to_owned(),
        owner: "presentation-document".to_owned(),
        persistence: "user-workspace".to_owned(),
        revision: workspace.presentation().revision.to_string(),
        sensitivity: "public".to_owned(),
        mutability: "presentation-only".to_owned(),
        activation: "none".to_owned(),
        fields: vec![
            conduit_patchbay::PatchbayConfigurationFieldProjection {
                id: "mode".to_owned(),
                display_value: workspace.presentation().mode.as_str().to_owned(),
            },
            conduit_patchbay::PatchbayConfigurationFieldProjection {
                id: "lens".to_owned(),
                display_value: workspace.presentation().lens.as_str().to_owned(),
            },
            conduit_patchbay::PatchbayConfigurationFieldProjection {
                id: "topology".to_owned(),
                display_value: workspace.presentation().topology.as_str().to_owned(),
            },
        ],
    });
    if let Some(run) = matching_run.as_ref() {
        configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
            id: "runtime/current".to_owned(),
            owner: "run".to_owned(),
            persistence: "run-epoch".to_owned(),
            revision: run.run_id.clone(),
            sensitivity: "bounded-projection".to_owned(),
            mutability: "runtime-state-read-only".to_owned(),
            activation: "runtime-control".to_owned(),
            fields: vec![conduit_patchbay::PatchbayConfigurationFieldProjection {
                id: "state".to_owned(),
                display_value: format!("{:?}", run.state).to_lowercase(),
            }],
        });
        configuration_layers.push(conduit_patchbay::PatchbayConfigurationLayerProjection {
            id: "evidence/immutable".to_owned(),
            owner: "evidence-stream".to_owned(),
            persistence: "evidence-retention-policy".to_owned(),
            revision: run.run_id.clone(),
            sensitivity: "bounded-redacted-projection".to_owned(),
            mutability: "immutable".to_owned(),
            activation: "none".to_owned(),
            fields: vec![conduit_patchbay::PatchbayConfigurationFieldProjection {
                id: "retained-events".to_owned(),
                display_value: evidence.len().to_string(),
            }],
        });
    }
    truncated |= configuration_layers.len() > bounds.maximum_configuration_layers;
    configuration_layers.truncate(bounds.maximum_configuration_layers);
    Ok(conduit_patchbay::PatchbayViewModel {
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_VERSION,
        source: workspace.source().clone(),
        semantic,
        presentation: workspace.presentation().clone(),
        plan,
        run: matching_run.clone(),
        high_water: matching_run.as_ref().and(high_water),
        evidence: if matching_run.is_some() {
            evidence
        } else {
            &[]
        }
        .iter()
        .take(bounds.maximum_evidence_events)
        .cloned()
        .collect(),
        topology: conduit_patchbay::PatchbayTopologyProjection {
            contract_imports: Vec::new(),
            logical_nodes,
            planned_realization,
            planned_realization_status,
            cords,
            composites,
            diagnostic_anchors,
            source_state: if document.ast.is_none()
                && recovered.nodes.iter().all(|node| node.complete)
                && recovered.cords.iter().all(|cord| cord.complete)
            {
                "invalid"
            } else {
                match recovered.state {
                    conduit_panel::RecoveredDocumentState::Exact if diagnostics.is_empty() => {
                        "exact"
                    }
                    conduit_panel::RecoveredDocumentState::Exact
                    | conduit_panel::RecoveredDocumentState::Invalid => "invalid",
                    conduit_panel::RecoveredDocumentState::Partial => "partial",
                }
            }
            .to_owned(),
        },
        at_rest: conduit_patchbay::project_at_rest(workspace.source()).ok(),
        configuration_layers,
        diagnostics,
        bounds,
        truncated: truncated || recovered.recovery_limited,
    })
}

fn project_authored_port(
    node_id: &str,
    port: &conduit_runtime::ResolvedPortView,
    direction: &str,
) -> conduit_patchbay::PatchbayPortProjection {
    let presentation_direction = if direction == "input" {
        "receiving"
    } else {
        "outgoing"
    };
    let node_path = if node_id.starts_with("root/") {
        node_id.to_owned()
    } else {
        format!("root/{node_id}")
    };
    conduit_patchbay::PatchbayPortProjection {
        id: port.id.clone(),
        semantic_path: format!("{node_path}/port/{presentation_direction}/{}", port.id),
        direction: direction.to_owned(),
        display_label: if direction == "input" {
            format!("> {}", port.id)
        } else {
            format!("{} >", port.id)
        },
        accessible_label: format!("{}, {presentation_direction} port", port.id),
        type_id: port.type_id.clone(),
        delivery: port.delivery.to_owned(),
        connections: port.connections.to_owned(),
        values: port.values.to_owned(),
        temporal: port.temporal.to_owned(),
        terminal: port.terminal.to_owned(),
        presence: port.presence.to_owned(),
        sensitivity: port.sensitivity.to_owned(),
        loss_acceptance: port.loss_acceptance.to_owned(),
        connected: false,
        source_range: None,
        validity: "valid".to_owned(),
        diagnostic_ids: Vec::new(),
    }
}

fn projected_port_direction(
    nodes: &[conduit_patchbay::PatchbayNodeProjection],
    node_id: &str,
    port_id: &str,
) -> Option<String> {
    let node = nodes.iter().find(|node| node.id == node_id)?;
    if node.inputs.iter().any(|port| port.id == port_id) {
        Some("input".to_owned())
    } else if node.outputs.iter().any(|port| port.id == port_id) {
        Some("output".to_owned())
    } else {
        None
    }
}

fn direction_path(direction: Option<&str>) -> &'static str {
    match direction {
        Some("input") => "receiving",
        Some("output") => "outgoing",
        _ => "unresolved",
    }
}

#[allow(clippy::too_many_arguments)]
fn add_patchbay_diagnostic(
    diagnostics: &mut Vec<conduit_patchbay::PatchbayDiagnosticProjection>,
    source_revision: u64,
    code: &str,
    severity: &str,
    state: &str,
    message: impl Into<String>,
    explanation: impl Into<String>,
    primary_range: Option<conduit_patchbay::SourceRangeProjection>,
    targets: Vec<(&str, &str)>,
) -> String {
    let id = format!("diagnostic/{source_revision}/{}/{code}", diagnostics.len());
    diagnostics.push(conduit_patchbay::PatchbayDiagnosticProjection {
        id: id.clone(),
        code: code.to_owned(),
        severity: severity.to_owned(),
        state: state.to_owned(),
        message: message.into(),
        explanation: explanation.into(),
        primary_range,
        related_ranges: Vec::new(),
        targets: targets
            .into_iter()
            .map(
                |(kind, id)| conduit_patchbay::PatchbayDiagnosticTargetProjection {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                },
            )
            .collect(),
    });
    id
}

fn add_diagnostic_anchor(
    anchors: &mut Vec<conduit_patchbay::PatchbayDiagnosticAnchorProjection>,
    cord: &conduit_panel::Cord,
    side: &str,
    owner_node: Option<String>,
    source_range: Option<conduit_patchbay::SourceRangeProjection>,
) -> String {
    let endpoint = if side == "from" { &cord.from } else { &cord.to };
    let id = format!("diagnostic-anchor/{}/{side}", cord.id);
    anchors.push(conduit_patchbay::PatchbayDiagnosticAnchorProjection {
        id: id.clone(),
        cord_id: cord.id.clone(),
        side: side.to_owned(),
        label: format!("{}.{}", endpoint.node, endpoint.port),
        owner_node,
        source_range,
    });
    id
}

fn add_recovered_anchor(
    anchors: &mut Vec<conduit_patchbay::PatchbayDiagnosticAnchorProjection>,
    cord: &conduit_panel::RecoveredCord,
    side: &str,
    owner_node: Option<String>,
    source_range: Option<conduit_patchbay::SourceRangeProjection>,
) -> String {
    let endpoint = if side == "from" { &cord.from } else { &cord.to };
    let id = format!("diagnostic-anchor/{}/{side}", cord.id);
    let label = match (&endpoint.node, &endpoint.port) {
        (Some(node), Some(port)) => format!("{node}.{port}"),
        (Some(node), None) => format!("{node}.…"),
        _ => "unfinished endpoint".to_owned(),
    };
    anchors.push(conduit_patchbay::PatchbayDiagnosticAnchorProjection {
        id: id.clone(),
        cord_id: cord.id.clone(),
        side: side.to_owned(),
        label,
        owner_node,
        source_range,
    });
    id
}

fn panel_without_recovered_cords(
    source: &str,
    recovered: &conduit_panel::RecoveredDocument,
) -> Option<conduit_panel::Panel> {
    let mut without_cords = source.as_bytes().to_vec();
    for cord in &recovered.cords {
        for byte in without_cords.get_mut(cord.source_span.start..cord.source_span.end)? {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    conduit_panel::parse(std::str::from_utf8(&without_cords).ok()?).ok()
}

fn recovered_cord_as_authored(
    source: &str,
    recovered: &conduit_panel::RecoveredCord,
) -> Option<conduit_panel::Cord> {
    let from = conduit_panel::Endpoint {
        node: recovered.from.node.clone()?,
        port: recovered.from.port.clone()?,
    };
    let to = conduit_panel::Endpoint {
        node: recovered.to.node.clone()?,
        port: recovered.to.port.clone()?,
    };
    let declaration = source.get(recovered.source_span.start..recovered.source_span.end)?;
    let capacity_items = authored_numeric_field::<u16>(declaration, "capacity").unwrap_or(8);
    let max_value_bytes =
        authored_numeric_field::<u32>(declaration, "max_value_bytes").unwrap_or(65_536);
    let max_queued_bytes = authored_numeric_field::<u64>(declaration, "max_queued_bytes")
        .unwrap_or(u64::from(capacity_items) * u64::from(max_value_bytes));
    let high_watermark_items =
        authored_numeric_field::<u16>(declaration, "high_watermark").unwrap_or(capacity_items);
    let low_watermark_items = authored_numeric_field::<u16>(declaration, "low_watermark")
        .unwrap_or(high_watermark_items.saturating_sub(1));
    let pressure_name = authored_field(declaration, "pressure").unwrap_or("block");
    let pressure = match pressure_name {
        "block" => conduit_panel::SourcePressure::Block,
        "reject" => conduit_panel::SourcePressure::Reject,
        "drop_disposable" | "drop-disposable" => conduit_panel::SourcePressure::DropDisposable,
        "disconnect" => conduit_panel::SourcePressure::Disconnect,
        "fail" => conduit_panel::SourcePressure::Fail,
        "coalesce" => conduit_panel::SourcePressure::Coalesce {
            relation: authored_field(declaration, "coalescer")?.to_owned(),
        },
        "sample" => conduit_panel::SourcePressure::Sample {
            every: authored_numeric_field(declaration, "sample_every")?,
            offset: authored_numeric_field(declaration, "sample_offset").unwrap_or(0),
        },
        _ => return None,
    };
    Some(conduit_panel::Cord {
        id: recovered.id.clone(),
        from,
        to,
        capacity_items,
        max_value_bytes,
        max_queued_bytes,
        low_watermark_items,
        high_watermark_items,
        pressure,
        operator_span: {
            let relative = declaration.find('>')?;
            let column = recovered.source_span.column + declaration[..relative].chars().count();
            conduit_panel::SourceSpan {
                line: recovered.source_span.line,
                column,
                end_line: recovered.source_span.line,
                end_column: column + 1,
            }
        },
        source_span: conduit_panel::SourceSpan {
            line: recovered.source_span.line,
            column: recovered.source_span.column,
            end_line: recovered.source_span.end_line,
            end_column: recovered.source_span.end_column,
        },
    })
}

fn authored_numeric_field<T>(source: &str, key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    authored_field(source, key)?.parse().ok()
}

fn authored_field<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let mut search = 0;
    while let Some(relative) = source.get(search..)?.find(key) {
        let start = search + relative;
        let before_ok = start == 0
            || !source
                .as_bytes()
                .get(start - 1)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
        let after = start + key.len();
        let after_ok = !source
            .as_bytes()
            .get(after)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
        if before_ok && after_ok {
            let tail = source.get(after..)?;
            let equals = tail.find('=')?;
            let value = tail.get(equals + 1..)?.trim_start();
            let end = value
                .find(|character: char| character.is_whitespace() || character == '}')
                .unwrap_or(value.len());
            return value.get(..end).filter(|value| !value.is_empty());
        }
        search = after;
    }
    None
}

fn invalid_cord_field(
    source: &str,
    cord: &conduit_panel::Cord,
    source_revision: u64,
) -> Option<(String, conduit_patchbay::SourceRangeProjection)> {
    let key = if cord.capacity_items == 0 {
        "capacity"
    } else if cord.max_value_bytes == 0 {
        "max_value_bytes"
    } else if cord.max_queued_bytes == 0 || cord.max_queued_bytes < u64::from(cord.max_value_bytes)
    {
        "max_queued_bytes"
    } else if cord.high_watermark_items == 0 || cord.high_watermark_items > cord.capacity_items {
        "high_watermark"
    } else if cord.low_watermark_items >= cord.high_watermark_items {
        "low_watermark"
    } else if matches!(
        cord.pressure,
        conduit_panel::SourcePressure::Sample { every: 0, .. }
    ) {
        "sample_every"
    } else {
        return None;
    };
    let declaration_start =
        source_range_for_span(source, cord.source_span, source_revision, "authored-cord")?
            .start_byte;
    let declaration = source.get(declaration_start..)?;
    let key_relative = declaration.find(key)?;
    let tail = declaration.get(key_relative + key.len()..)?;
    let equals = tail.find('=')?;
    let value = tail.get(equals + 1..)?;
    let leading = value.len() - value.trim_start().len();
    let value = value.trim_start();
    let value_end = value
        .find(|character: char| character.is_whitespace() || character == '}')
        .unwrap_or(value.len());
    let start = declaration_start + key_relative;
    let end = declaration_start + key_relative + key.len() + equals + 1 + leading + value_end;
    Some((
        format!("{}/{}", cord.id, key),
        source_range_from_offsets(source, (start, end), source_revision, "authored-cord-field")?,
    ))
}

fn mark_connected_ports(
    nodes: &mut [conduit_patchbay::PatchbayNodeProjection],
    cords: &[conduit_patchbay::PatchbayCordProjection],
) {
    for node in nodes {
        for port in &mut node.inputs {
            port.connected = cords.iter().any(|cord| {
                cord.to_node.as_deref() == Some(node.id.as_str())
                    && cord.to_port.as_deref() == Some(port.id.as_str())
            });
        }
        for port in &mut node.outputs {
            port.connected = cords.iter().any(|cord| {
                cord.from_node.as_deref() == Some(node.id.as_str())
                    && cord.from_port.as_deref() == Some(port.id.as_str())
            });
        }
    }
}

fn source_range_for_recovered_span(
    source: &str,
    span: conduit_panel::Span,
    source_revision: u64,
    provenance: &str,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    source_range_from_offsets(source, (span.start, span.end), source_revision, provenance)
}

fn source_range_for_span(
    source: &str,
    span: conduit_panel::SourceSpan,
    source_revision: u64,
    provenance: &str,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    fn byte_offset(source: &str, line: usize, column: usize) -> Option<usize> {
        if line == 0 || column == 0 {
            return None;
        }
        let mut byte = 0;
        for _ in 1..line {
            byte += source.get(byte..)?.find('\n')? + 1;
        }
        let line_end = source[byte..]
            .find('\n')
            .map_or(source.len(), |relative| byte + relative);
        let relative = source[byte..line_end]
            .char_indices()
            .nth(column - 1)
            .map_or(line_end - byte, |(relative, _)| relative);
        Some(byte + relative)
    }
    source_range_from_offsets(
        source,
        (
            byte_offset(source, span.line, span.column)?,
            byte_offset(source, span.end_line, span.end_column)?,
        ),
        source_revision,
        provenance,
    )
}

fn parse_error_source_range(
    source: &str,
    error: &conduit_panel::ParseError,
    source_revision: u64,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    let span = conduit_panel::SourceSpan {
        line: error.line,
        column: error.column,
        end_line: error.line,
        end_column: error.column.saturating_add(1),
    };
    source_range_for_span(source, span, source_revision, "parser-diagnostic")
}

fn project_config_value(
    value: &conduit_panel::SourceValue,
    source_revision: u64,
) -> conduit_patchbay::PatchbayConfigProjection {
    let (kind, display_value, editable) = match value {
        conduit_panel::SourceValue::Boolean(value) => ("boolean", value.to_string(), false),
        conduit_panel::SourceValue::Integer(value) => ("integer", value.to_string(), false),
        conduit_panel::SourceValue::Text(value) => ("text", value.clone(), true),
        conduit_panel::SourceValue::Bytes(value) => {
            ("bytes", format!("[{} bytes]", value.len()), false)
        }
        conduit_panel::SourceValue::Reference(value) => ("reference", value.clone(), false),
        conduit_panel::SourceValue::ContractReference(value) => {
            ("contract-reference", value.clone(), false)
        }
        conduit_panel::SourceValue::SecretReference(_) => {
            ("secret-reference", "[redacted]".to_owned(), false)
        }
        conduit_panel::SourceValue::ExactDecimal(value) => ("exact-decimal", value.clone(), false),
        conduit_panel::SourceValue::List(values) => {
            ("list", format!("[{} values]", values.len()), false)
        }
        conduit_panel::SourceValue::Record(fields) => {
            ("record", format!("{{{} fields}}", fields.len()), false)
        }
    };
    conduit_patchbay::PatchbayConfigProjection {
        kind: kind.to_owned(),
        display_value,
        editable,
        owner: "panel-instance".to_owned(),
        persistence: "source-document".to_owned(),
        revision: source_revision,
        sensitivity: if matches!(value, conduit_panel::SourceValue::SecretReference(_)) {
            "secret"
        } else {
            "public"
        }
        .to_owned(),
        activation: "source-candidate-requires-resolution".to_owned(),
        source_range: None,
        validity: "valid".to_owned(),
        diagnostic_ids: Vec::new(),
    }
}

/// Applies a source transaction through the production Patchbay protocol.
/// The browser receives only the separate source/semantic/presentation facts.
#[wasm_bindgen]
pub fn patchbay_replace_source(source: String, replacement: String) -> String {
    let mut workspace = match conduit_patchbay::Workspace::new("tour/draft", source) {
        Ok(workspace) => workspace,
        Err(error) => return patchbay_error(error),
    };
    let request = conduit_patchbay::EditRequest {
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_VERSION,
        document_id: "tour/draft".to_owned(),
        expected_source_revision: workspace.source().revision,
        expected_presentation_revision: workspace.presentation().revision,
        operations: vec![conduit_patchbay::EditOperation::ReplaceSource {
            source: replacement,
        }],
    };
    match workspace.apply(request) {
        Ok(result) => patchbay_result(result),
        Err(error) => patchbay_error(error),
    }
}

/// Applies a presentation-only visual move through the same Patchbay protocol.
#[wasm_bindgen]
pub fn patchbay_move_node(source: String, node_id: String, x: i32, y: i32) -> String {
    let mut workspace = match conduit_patchbay::Workspace::new("tour/draft", source) {
        Ok(workspace) => workspace,
        Err(error) => return patchbay_error(error),
    };
    let request = conduit_patchbay::EditRequest {
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_VERSION,
        document_id: "tour/draft".to_owned(),
        expected_source_revision: workspace.source().revision,
        expected_presentation_revision: workspace.presentation().revision,
        operations: vec![conduit_patchbay::EditOperation::MoveNode {
            node_id,
            position: conduit_patchbay::NodePosition { x, y },
        }],
    };
    match workspace.apply(request) {
        Ok(result) => patchbay_result(result),
        Err(error) => patchbay_error(error),
    }
}

/// Returns a small JSON summary produced from `conduit_panel::parse` itself.
#[wasm_bindgen]
pub fn parse_panel(source: String) -> String {
    match conduit_panel::parse(&source) {
        Ok(panel) => format!(
            "{{\"ok\":true,\"nodes\":{},\"cords\":{},\"node_labels\":[{}]}}",
            panel.nodes.len(),
            panel.cords.len(),
            panel
                .nodes
                .iter()
                .map(|node| format!("{:?}", format!("{}: {}", node.id, node.kind)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Err(error) => format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    }
}

/// Returns parser-owned lexical metadata for browser source presentation.
#[wasm_bindgen]
pub fn panel_language_metadata() -> String {
    serde_json::json!({
        "schema": "conduit.panel-language",
        "reserved_words": conduit_panel::RESERVED_WORDS,
        "syntax_words": conduit_panel::SYNTAX_WORDS,
        "identifier_compatible_syntax_words":
            conduit_panel::IDENTIFIER_COMPATIBLE_SYNTAX_WORDS,
    })
    .to_string()
}

fn source_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let mut byte = 0;
    for _ in 1..line {
        byte += source.get(byte..)?.find('\n')? + 1;
    }
    let line_end = source[byte..]
        .find('\n')
        .map_or(source.len(), |relative| byte + relative);
    let relative = source[byte..line_end]
        .char_indices()
        .nth(column - 1)
        .map_or(line_end - byte, |(relative, _)| relative);
    Some(byte + relative)
}

fn source_span_offsets(source: &str, span: conduit_panel::SourceSpan) -> Option<(usize, usize)> {
    let start = source_offset(source, span.line, span.column)?;
    let end = source_offset(source, span.end_line, span.end_column)?;
    (start <= end && end <= source.len()).then_some((start, end))
}

fn source_range_from_offsets(
    source: &str,
    (start_byte, end_byte): (usize, usize),
    source_revision: u64,
    provenance: &str,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    if start_byte >= end_byte
        || end_byte > source.len()
        || !source.is_char_boundary(start_byte)
        || !source.is_char_boundary(end_byte)
    {
        return None;
    }
    Some(conduit_patchbay::SourceRangeProjection {
        start_byte,
        end_byte,
        start_utf16: source[..start_byte].encode_utf16().count(),
        end_utf16: source[..end_byte].encode_utf16().count(),
        source_revision,
        provenance: provenance.to_owned(),
    })
}

fn is_panel_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/' | b'@' | b'[' | b']')
}

fn named_member_offset(source: &str, start: usize, end: usize, id: &str) -> Option<usize> {
    let declaration = source.get(start..end)?;
    declaration.match_indices(id).find_map(|(relative, _)| {
        let absolute = start + relative;
        let before = absolute
            .checked_sub(1)
            .and_then(|index| source.as_bytes().get(index))
            .is_some_and(|byte| is_panel_identifier_byte(*byte));
        let after = source
            .as_bytes()
            .get(absolute + id.len())
            .is_some_and(|byte| is_panel_identifier_byte(*byte));
        (!before && !after).then_some(absolute)
    })
}

fn annotation(
    source: &str,
    start_byte: usize,
    end_byte: usize,
    kind: &str,
    direction: &str,
    semantic_path: String,
) -> serde_json::Value {
    serde_json::json!({
        "start_byte": start_byte,
        "end_byte": end_byte,
        "start_utf16": source[..start_byte].encode_utf16().count(),
        "end_utf16": source[..end_byte].encode_utf16().count(),
        "kind": kind,
        "direction": direction,
        "semantic_path": semantic_path,
        "accessible_label": match kind {
            "port-name" => format!("{}, {} port", &source[start_byte..end_byte], direction),
            "port-sigil" => format!("{} port declaration sigil", direction),
            _ => format!("{} port", direction),
        },
    })
}

fn direction_name(direction: conduit_panel::ExportDirection) -> &'static str {
    match direction {
        conduit_panel::ExportDirection::Input => "receiving",
        conduit_panel::ExportDirection::Output => "outgoing",
    }
}

fn annotate_directional_declaration(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    span: conduit_panel::SourceSpan,
    id: &str,
    direction: conduit_panel::ExportDirection,
    semantic_path: String,
) {
    let Some((start, end)) = source_span_offsets(source, span) else {
        return;
    };
    let Some(id_start) = named_member_offset(source, start, end, id) else {
        return;
    };
    let direction = direction_name(direction);
    annotations.push(annotation(
        source,
        id_start,
        id_start + id.len(),
        "port-name",
        direction,
        semantic_path.clone(),
    ));

    let before = source.get(start..id_start).unwrap_or_default();
    let after = source.get(id_start + id.len()..end).unwrap_or_default();
    // The parser establishes direction. Source presentation may put either
    // flow sigil before or after the declared name, so locate its authored
    // position rather than inferring one from the parser direction.
    let sigil = before
        .rfind(['>', '<'])
        .map(|relative| start + relative)
        .or_else(|| {
            after
                .find(['>', '<'])
                .map(|relative| id_start + id.len() + relative)
        });
    if let Some(sigil_start) = sigil {
        annotations.push(annotation(
            source,
            sigil_start,
            sigil_start + 1,
            "port-sigil",
            direction,
            semantic_path,
        ));
    }
}

fn annotate_endpoint(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    search_range: (usize, usize),
    endpoint: &conduit_panel::Endpoint,
    direction: &str,
    semantic_path: String,
    reverse: bool,
) {
    let Some((member_start, member_end)) =
        endpoint_member_offset(source, search_range, endpoint, reverse)
    else {
        return;
    };
    annotations.push(annotation(
        source,
        member_start,
        member_end,
        "port-name",
        direction,
        semantic_path,
    ));
}

fn endpoint_member_offset(
    source: &str,
    (search_start, search_end): (usize, usize),
    endpoint: &conduit_panel::Endpoint,
    reverse: bool,
) -> Option<(usize, usize)> {
    let complete = if endpoint.port.is_empty() {
        endpoint.node.clone()
    } else {
        format!("{}.{}", endpoint.node, endpoint.port)
    };
    let region = source.get(search_start..search_end)?;
    let relative = if reverse {
        region.rfind(&complete)
    } else {
        region.find(&complete)
    }?;
    let member = if endpoint.port.is_empty() {
        endpoint.node.as_str()
    } else {
        endpoint.port.as_str()
    };
    let member_start = search_start + relative + complete.len() - member.len();
    Some((member_start, member_start + member.len()))
}

fn endpoint_source_range(
    source: &str,
    member_range: (usize, usize),
    endpoint: &conduit_panel::Endpoint,
    source_revision: u64,
    provenance: &str,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    let endpoint_text = if endpoint.port.is_empty() {
        endpoint.node.clone()
    } else {
        format!("{}.{}", endpoint.node, endpoint.port)
    };
    let start = if endpoint.port.is_empty() {
        member_range.0
    } else {
        member_range.0.checked_sub(endpoint.node.len() + 1)?
    };
    if source.get(start..member_range.1)? != endpoint_text {
        return None;
    }
    source_range_from_offsets(source, (start, member_range.1), source_revision, provenance)
}

fn cord_endpoint_member_offsets(
    source: &str,
    cord: &conduit_panel::Cord,
) -> Option<((usize, usize), (usize, usize))> {
    let (start, end) = source_span_offsets(source, cord.source_span)?;
    let declaration = source.get(start..end)?;
    let body_start = declaration.find('{').unwrap_or(declaration.len());
    let endpoints = &declaration[..body_start];
    let relative = endpoints.find('>')?;
    let operator = start + relative;
    let from = endpoint_member_offset(source, (start, operator), &cord.from, true)?;
    let to = endpoint_member_offset(source, (operator + 1, start + body_start), &cord.to, false)?;
    Some((from, to))
}

fn annotate_cords(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    owner: &str,
    cords: &[conduit_panel::Cord],
) {
    for cord in cords {
        if let Some((start, end)) = source_span_offsets(source, cord.operator_span) {
            annotations.push(annotation(
                source,
                start,
                end,
                "graph-operator",
                "connect",
                format!("{owner}/cord/{}/operator", cord.id),
            ));
        }
        let Some((from_range, to_range)) = cord_endpoint_member_offsets(source, cord) else {
            continue;
        };
        let cord_path = format!("{owner}/cord/{}", cord.id);
        annotations.push(annotation(
            source,
            from_range.0,
            from_range.1,
            "port-name",
            "outgoing",
            format!("{cord_path}/from/{}/{}", cord.from.node, cord.from.port),
        ));
        annotations.push(annotation(
            source,
            to_range.0,
            to_range.1,
            "port-name",
            "receiving",
            format!("{cord_path}/to/{}/{}", cord.to.node, cord.to.port),
        ));
    }
}

fn annotate_expression(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    owner: &str,
    expression: &conduit_panel::SourceExpression,
) {
    if let conduit_panel::SourceExpression::Binary {
        operation,
        left,
        right,
        operator_span,
    } = expression
    {
        if let Some((start, end)) = source_span_offsets(source, *operator_span) {
            annotations.push(annotation(
                source,
                start,
                end,
                "expression-operator",
                match operation {
                    conduit_panel::ExpressionOperator::Add => "add",
                    conduit_panel::ExpressionOperator::Subtract => "subtract",
                    conduit_panel::ExpressionOperator::Multiply => "multiply",
                    conduit_panel::ExpressionOperator::Divide => "divide",
                    conduit_panel::ExpressionOperator::LessThan => "less-than",
                    conduit_panel::ExpressionOperator::LessThanOrEqual => "less-than-or-equal",
                    conduit_panel::ExpressionOperator::GreaterThan => "greater-than",
                    conduit_panel::ExpressionOperator::GreaterThanOrEqual => {
                        "greater-than-or-equal"
                    }
                    conduit_panel::ExpressionOperator::Equal => "equal",
                    conduit_panel::ExpressionOperator::NotEqual => "not-equal",
                },
                format!("{owner}/expression/operator"),
            ));
        }
        annotate_expression(source, annotations, owner, left);
        annotate_expression(source, annotations, owner, right);
    }
}

fn annotate_export_target(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    definition_id: &str,
    export: &conduit_panel::PortExport,
) {
    let Some((start, end)) = source_span_offsets(source, export.source_span) else {
        return;
    };
    let declaration = source.get(start..end).unwrap_or_default();
    let target_start = declaration
        .find('=')
        .map_or(start, |relative| start + relative + 1);
    let direction = direction_name(export.direction);
    annotate_endpoint(
        source,
        annotations,
        (target_start, end),
        &export.target,
        direction,
        format!(
            "definition/{definition_id}/export/{}/target/{}/{}",
            export.id, export.target.node, export.target.port
        ),
        false,
    );
}

/// Returns exact semantic port ranges derived from the production parser.
///
/// Malformed source deliberately returns no semantic annotations: the browser
/// may retain lossless lexical presentation, but it must not guess direction.
#[wasm_bindgen]
pub fn panel_source_metadata(source: String) -> String {
    let document = conduit_panel::parse_document(&source);
    let Ok(panel) = document.panel() else {
        return serde_json::json!({
            "schema": "conduit.panel-source-metadata",
            "semantic_available": false,
            "annotations": [],
            "diagnostics": document.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })
        .to_string();
    };

    let mut annotations = Vec::new();
    for interface in &panel.interfaces {
        for member in &interface.members {
            annotate_directional_declaration(
                &source,
                &mut annotations,
                member.source_span,
                &member.id,
                member.direction,
                format!(
                    "interface/{}/port/{}/{}",
                    interface.id,
                    direction_name(member.direction),
                    member.id
                ),
            );
        }
    }
    for group in &panel.port_groups {
        annotate_directional_declaration(
            &source,
            &mut annotations,
            group.source_span,
            &group.id,
            group.direction,
            format!(
                "root/port-group/{}/{}",
                direction_name(group.direction),
                group.id
            ),
        );
    }
    annotate_cords(&source, &mut annotations, "root", &panel.cords);
    for node in &panel.nodes {
        if let Some(expression) = &node.expression {
            annotate_expression(
                &source,
                &mut annotations,
                &format!("root/node/{}", node.id),
                expression,
            );
        }
    }
    for definition in &panel.definitions {
        for export in &definition.exports {
            annotate_directional_declaration(
                &source,
                &mut annotations,
                export.source_span,
                &export.id,
                export.direction,
                format!(
                    "definition/{}/port/{}/{}",
                    definition.id,
                    direction_name(export.direction),
                    export.id
                ),
            );
            annotate_export_target(&source, &mut annotations, &definition.id, export);
        }
        for group in &definition.port_groups {
            annotate_directional_declaration(
                &source,
                &mut annotations,
                group.source_span,
                &group.id,
                group.direction,
                format!(
                    "definition/{}/port-group/{}/{}",
                    definition.id,
                    direction_name(group.direction),
                    group.id
                ),
            );
        }
        annotate_cords(
            &source,
            &mut annotations,
            &format!("definition/{}", definition.id),
            &definition.cords,
        );
        for node in &definition.nodes {
            if let Some(expression) = &node.expression {
                annotate_expression(
                    &source,
                    &mut annotations,
                    &format!("definition/{}/node/{}", definition.id, node.id),
                    expression,
                );
            }
        }
    }
    annotations.sort_by_key(|value| {
        (
            value["start_byte"].as_u64().unwrap_or(u64::MAX),
            value["end_byte"].as_u64().unwrap_or(u64::MAX),
        )
    });
    serde_json::json!({
        "schema": "conduit.panel-source-metadata",
        "semantic_available": true,
        "annotations": annotations,
        "diagnostics": [],
    })
    .to_string()
}

/// Returns the production resolver's logical and expanded projections.
#[wasm_bindgen]
pub fn explain_panel(source: String) -> String {
    let panel = match conduit_panel::parse(&source) {
        Ok(panel) => panel,
        Err(error) => {
            return serde_json::json!({
                "ok": false,
                "diagnostic": error.to_string(),
            })
            .to_string();
        }
    };
    match browser_registry().resolve(&panel) {
        Ok(resolved) => serde_json::json!({
            "ok": true,
            "logical": resolved.explain_logical(),
            "expanded": resolved.explain_expanded(),
        })
        .to_string(),
        Err(error) => serde_json::json!({
            "ok": false,
            "diagnostic": error.to_string(),
        })
        .to_string(),
    }
}

/// Executes the finite hosted compatibility demo with bounded in-memory streams.
#[wasm_bindgen]
pub fn run_panel_compatibility_demo(source: String) -> String {
    let panel = match conduit_panel::parse(&source) {
        Ok(panel) => panel,
        Err(error) => return format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    };
    let registry = conduit_runtime::Registry::compatibility_demo();
    let resolved = match registry.resolve(&panel) {
        Ok(resolved) => resolved,
        Err(error) => return format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    };
    let mut input = std::io::empty();
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let mut io = conduit_runtime::RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
        display: &mut display,
    };
    match resolved.run_batch(&mut io) {
        Ok(summary) => format!(
            "{{\"ok\":true,\"completed_nodes\":{},\"cords_conducted\":{},\"stdout\":{:?},\"stderr\":{:?},\"display\":{:?}}}",
            summary.nodes_completed,
            summary.cords_conducted,
            String::from_utf8_lossy(&output),
            String::from_utf8_lossy(&error),
            String::from_utf8_lossy(&display)
        ),
        Err(error) => format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    }
}

/// Compiles immutable inputs and executes their exact plan through the same
/// bounded deterministic executor used by `conduct run`.
#[wasm_bindgen]
pub fn run_panel_exact(source: String, compile_input_json: String) -> String {
    run_panel_result(&source, Some(&compile_input_json))
}

/// Observes compiled-in browser providers and executes their exact plan.
#[wasm_bindgen]
pub fn run_panel(source: String) -> String {
    run_panel_result(&source, None)
}

fn run_panel_result(source: &str, compile_input_json: Option<&str>) -> String {
    match run_panel_exact_inner(source, compile_input_json, None) {
        Ok(result) => serde_json::json!({
            "ok": true,
            "terminal": terminal_name(result.report.terminal),
            "completed_nodes": result.report.summary.nodes_completed,
            "cords_conducted": result.report.summary.cords_conducted,
            "stdout": String::from_utf8_lossy(&result.output),
            "stderr": String::from_utf8_lossy(&result.error),
            "display": String::from_utf8_lossy(&result.display),
            "profile": "exact-plan-deterministic-executor",
            "high_water": {
                "queue_items": result.report.high_water.queue_items,
                "queue_payload_bytes": result.report.high_water.queue_payload_bytes,
                "ready_slots": result.report.high_water.ready_slots,
                "event_slots": result.report.high_water.event_slots,
                "decisions": result.report.high_water.decisions,
            },
            "scheduler_event_count": result.report.scheduler_events.len(),
            "evidence_bytes": result.report.evidence_bytes,
            "evidence": result.report.evidence,
            "patchbay": result.patchbay,
        })
        .to_string(),
        Err(error) => serde_json::json!({
            "ok": false,
            "code": error.code,
            "diagnostic": error.to_string(),
        })
        .to_string(),
    }
}

/// Starts the production executor and applies deterministic abort
/// cancellation before its first node step.
#[wasm_bindgen]
pub fn cancel_panel(source: String) -> String {
    match run_panel_exact_inner(&source, None, Some(conduit_core::StopPolicy::Abort)) {
        Ok(result) => serde_json::json!({
            "ok": true,
            "terminal": terminal_name(result.report.terminal),
            "stdout": String::from_utf8_lossy(&result.output),
            "stderr": String::from_utf8_lossy(&result.error),
            "profile": "exact-plan-deterministic-executor",
            "evidence_bytes": result.report.evidence_bytes,
            "evidence": result.report.evidence,
            "patchbay": result.patchbay,
        })
        .to_string(),
        Err(error) => serde_json::json!({
            "ok": false,
            "code": error.code,
            "diagnostic": error.to_string(),
        })
        .to_string(),
    }
}

fn run_panel_exact_inner(
    source: &str,
    compile_input_json: Option<&str>,
    initial_stop: Option<conduit_core::StopPolicy>,
) -> Result<ExactBrowserResult, RuntimeError> {
    let panel = conduit_panel::parse(source)
        .map_err(|error| RuntimeError::new("CND-SRC-001", error.to_string()))?;
    let registry = browser_registry();
    let topology = registry
        .resolve(&panel)
        .and_then(|resolved| resolved.exact_topology())
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    let installed = InstalledProfile::observe_registry_on_host(
        source,
        &registry,
        &browser_host_observation(),
        &[],
    )?
    .with_implementation_preference(vec![
        conduit_audio::transform_implementations::MediaImplementation::BrowserWasmLinked
            .id()
            .to_owned(),
    ])?
    .with_evidence_provider_observation(browser_evidence_provider_observation())?
    .with_watch_admissions(browser_watch_admissions(&topology))?;
    let explicit_input = compile_input_json
        .map(|json| {
            serde_json::from_str::<CompileInput>(json)
                .map_err(|_| RuntimeError::new("CND-CMP-002", "invalid compile-input JSON"))
        })
        .transpose()?;
    let input = explicit_input.as_ref().unwrap_or(&installed.input);
    let document = compile_source(source, input)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let arena = bumpalo::Bump::new();
    let plan = document
        .as_plan(&arena)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let bindings = installed.bindings(&plan)?;
    let grant_observations = installed.grant_observations(&plan)?;
    let mut plan_snapshot = conduit_patchbay::PlanSnapshot::from_exact_plan(&plan);
    let workspace = conduit_patchbay::Workspace::new("conduit/browser-source", source)
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))?;
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    pin_plan_semantic_promises(&mut plan_snapshot, &resolved.view());
    let mut input_stream = std::io::empty();
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let report = {
        let mut io = conduit_runtime::RunIo {
            input: &mut input_stream,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        let context = ExactRunContext {
            semantic_source_hash: plan.source_semantic_hash,
            plan_epoch: 1,
            run_id: conduit_core::Id("conduit/browser-run"),
            grant_observations: &grant_observations,
            validation: conduit_core::PlanValidationContext {
                supported_schema_version: plan.schema_version,
                now: plan.created_at,
            },
            scheduler_policy: SchedulerPolicy {
                schema_version: SCHEDULER_CONTRACT_VERSION,
                ready_queue: ReadyQueueDiscipline::RoundRobin,
                max_decisions: 256,
                max_tick: 512,
                max_consecutive_yields: 8,
                max_events: if plan.nodes.len() > 4 { 256 } else { 128 },
            },
            reservation: SchedulerReservation {
                available_runtime_memory_bytes: plan.budget.memory_bytes,
                executor_overhead_limit_bytes: plan.budget.memory_bytes,
            },
        };
        match initial_stop {
            Some(stop) => resolved.cancel_exact_report(&plan, &bindings, context, stop, &mut io)?,
            None => resolved.run_exact_report(&plan, &bindings, context, &mut io)?,
        }
    };
    let run = conduit_patchbay::RunSnapshot {
        run_id: "conduit/browser-run".to_owned(),
        plan_identity: plan_snapshot.identity.clone(),
        source_semantic_hash: plan_snapshot.source_semantic_hash.clone(),
        state: conduit_patchbay::RunState::Terminal,
    };
    let evidence = report
        .evidence
        .iter()
        .map(|event| {
            serde_json::to_value(event)
                .map_err(|error| RuntimeError::new("CND-PBY-009", error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let patchbay = serde_json::to_value(
        authoritative_patchbay_view(
            &workspace,
            Some(plan_snapshot),
            Some(run),
            Some(conduit_patchbay::PatchbayHighWaterProjection {
                queue_items: report.high_water.queue_items,
                queue_payload_bytes: report.high_water.queue_payload_bytes,
                ready_slots: report.high_water.ready_slots,
                event_slots: report.high_water.event_slots,
                decisions: report.high_water.decisions,
            }),
            &evidence,
        )
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))?,
    )
    .map_err(|error| RuntimeError::new("CND-PBY-009", error.to_string()))?;
    Ok(ExactBrowserResult {
        report,
        output,
        error,
        display,
        patchbay,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use conduit_core::{SemanticHash, Sensitivity};
    use conduit_runtime::{
        ExactWatchMaterial, ExactWatchObservation, ExactWatchSubject, ExactWatchTimestamp,
    };
    use serde_json::Value;

    use super::{
        authoritative_patchbay_view, browser_exact_run_id, browser_watch_observation,
        exact_plan_snapshot, explain_panel, panel_language_metadata, panel_source_metadata,
        patchbay_advance_exact_run, patchbay_apply_transaction, patchbay_attach_exact_watch,
        patchbay_cancel_exact_run, patchbay_detach_exact_watch, patchbay_dispose_exact_run,
        patchbay_inspect_at_rest, patchbay_move_node, patchbay_notify_host_operation,
        patchbay_open_session, patchbay_pump_exact_run, patchbay_read_exact_evidence,
        patchbay_read_exact_watch, patchbay_replace_source, patchbay_session_view,
        patchbay_snapshot_exact_run, patchbay_start_exact_run, planned_realization_projection,
    };

    const SOURCE: &str = "panel 0\ngreeting: std/literal { value = \"hello\\n\" }\noutput: display/text\ngreeting.value > output.text\n";

    #[derive(Clone)]
    struct TestRunBinding {
        run_id: String,
        source_revision: u64,
        plan_identity: String,
    }

    fn test_run_binding(started: &Value) -> TestRunBinding {
        TestRunBinding {
            run_id: started["run_id"].as_str().expect("run identity").to_owned(),
            source_revision: started["source_revision"]
                .as_u64()
                .expect("source revision"),
            plan_identity: started["plan_identity"]
                .as_str()
                .expect("plan identity")
                .to_owned(),
        }
    }

    macro_rules! bound_run {
        (patchbay_attach_exact_watch, $session:expr, $binding:expr, $watch:expr $(,)?) => {
            patchbay_attach_exact_watch(
                $session.to_owned(),
                $binding.run_id.clone(),
                $binding.source_revision,
                $binding.plan_identity.clone(),
                super::BROWSER_WATCH_OPERATOR.to_owned(),
                $watch,
            )
        };
        (patchbay_detach_exact_watch, $session:expr, $binding:expr, $watch:expr $(,)?) => {
            patchbay_detach_exact_watch(
                $session.to_owned(),
                $binding.run_id.clone(),
                $binding.source_revision,
                $binding.plan_identity.clone(),
                super::BROWSER_WATCH_OPERATOR.to_owned(),
                $watch,
            )
        };
        (patchbay_read_exact_watch, $session:expr, $binding:expr, $watch:expr, $cursor:expr, $maximum:expr $(,)?) => {
            patchbay_read_exact_watch(
                $session.to_owned(),
                $binding.run_id.clone(),
                $binding.source_revision,
                $binding.plan_identity.clone(),
                super::BROWSER_WATCH_OPERATOR.to_owned(),
                $watch,
                $cursor,
                $maximum,
            )
        };
        ($function:ident, $session:expr, $binding:expr $(, $argument:expr)* $(,)?) => {
            $function(
                $session.to_owned(),
                $binding.run_id.clone(),
                $binding.source_revision,
                $binding.plan_identity.clone()
                $(, $argument)*
            )
        };
    }

    fn watch_observation(
        representation_id: &str,
        bytes: Vec<u8>,
        truncated: bool,
    ) -> ExactWatchObservation {
        ExactWatchObservation {
            cursor: 3,
            source_sequence: 7,
            tick: 11,
            watch_id: "watch/cord-0".to_owned(),
            subject: ExactWatchSubject::Cord {
                cord: "cord-0".to_owned(),
            },
            producing_host: "conduit/producer-host".to_owned(),
            host_observation: "conduit/producer-observation".to_owned(),
            time_basis: "clock/producer".to_owned(),
            clock_uncertainty_ticks: 0,
            value_timestamps: vec![ExactWatchTimestamp {
                clock_domain: "clock/value".to_owned(),
                tick: 10,
                uncertainty_ticks: 2,
            }],
            value_handle: 4,
            accounted_bytes: u32::try_from(bytes.len()).unwrap(),
            representation_id: representation_id.to_owned(),
            representation_schema_version: 0,
            representation_semantic_hash: SemanticHash::from_bytes([8; 32]),
            sensitivity: Sensitivity::Public,
            value_identity: Some(SemanticHash::from_bytes([9; 32])),
            provenance: Some(SemanticHash::from_bytes([10; 32])),
            content_hash: Some(SemanticHash::from_bytes([11; 32])),
            original_bytes: u32::try_from(bytes.len()).unwrap(),
            truncated,
            gap_before: 0,
            material: ExactWatchMaterial::Preview(bytes),
        }
    }

    #[test]
    fn exact_watch_projection_uses_only_pinned_renderers_and_keeps_bytes_as_fallback() {
        let closed_record = vec![
            0, 1, // one field
            0, 4, b'n', b'a', b'm', b'e', // field name
            0, 0, 0, 3, 0, 1, 255, // opaque field bytes
        ];
        let record =
            browser_watch_observation(&watch_observation("std/record", closed_record, false));
        assert_eq!(
            record["material"]["renderer"]["id"],
            "conduit.browser/closed-record-fields"
        );
        assert_eq!(record["material"]["record"]["fields"][0]["name"], "name");
        assert_eq!(
            record["material"]["record"]["fields"][0]["bytes"],
            serde_json::json!([0, 1, 255])
        );
        assert_eq!(record["producing_host"], "conduit/producer-host");
        assert_eq!(record["time_basis"], "clock/producer");
        assert_eq!(record["clock_uncertainty_ticks"], 0);
        assert_eq!(record["value_timestamps"][0]["clock_domain"], "clock/value");
        assert_eq!(record["value_timestamps"][0]["uncertainty_ticks"], 2);

        for (representation, kind, renderer) in [
            (
                "conduit.media/audio-frame",
                "audio",
                "conduit.browser/audio-frame-summary",
            ),
            (
                "conduit.media/video-frame",
                "image",
                "conduit.browser/video-frame-summary",
            ),
        ] {
            let derived =
                browser_watch_observation(&watch_observation(representation, vec![1, 2, 3], false));
            assert_eq!(derived["material"]["derived"]["kind"], kind);
            assert_eq!(derived["material"]["renderer"]["id"], renderer);
            assert_eq!(
                derived["material"]["renderer"]["derivation"],
                "bounded-byte-summary"
            );
        }

        let binary = browser_watch_observation(&watch_observation(
            "example/unknown-binary",
            vec![0, 255],
            false,
        ));
        assert_eq!(binary["material"]["bytes"], serde_json::json!([0, 255]));
        assert_eq!(binary["material"]["renderer"]["status"], "missing");
        assert!(binary["material"]["renderer"]["id"].is_null());

        let mut truncated_text = watch_observation("std/text", b"prefix".to_vec(), true);
        truncated_text.original_bytes = 4_096;
        let truncated = browser_watch_observation(&truncated_text);
        assert_eq!(truncated["truncated"], true);
        assert_eq!(truncated["original_bytes"], 4_096);
        assert_eq!(
            truncated["content_hash"],
            SemanticHash::from_bytes([11; 32]).to_string()
        );
        assert_eq!(truncated["material"]["text"], "prefix");

        let mut protected = watch_observation("std/text", Vec::new(), false);
        protected.sensitivity = Sensitivity::Restricted;
        protected.content_hash = None;
        protected.original_bytes = 42;
        protected.material = ExactWatchMaterial::Redacted;
        let redacted = browser_watch_observation(&protected);
        assert_eq!(redacted["material"]["kind"], "redacted");
        assert_eq!(redacted["subject"]["cord"], "cord-0");
        assert_eq!(redacted["representation"]["id"], "std/text");
        assert_eq!(redacted["original_bytes"], 42);
        assert!(redacted["content_hash"].is_null());
    }

    #[test]
    fn parser_metadata_and_patchbay_ranges_are_authoritative() {
        let metadata: Value =
            serde_json::from_str(&panel_language_metadata()).expect("language metadata JSON");
        assert_eq!(metadata["schema"], "conduit.panel-language");
        assert_eq!(metadata["reserved_words"], serde_json::json!([]));
        assert_eq!(
            metadata["identifier_compatible_syntax_words"],
            serde_json::json!(["input", "output"])
        );
        assert!(
            metadata["syntax_words"]
                .as_array()
                .is_some_and(|words| words.contains(&Value::String("output".to_owned())))
        );

        let source = SOURCE.replace("hello", "héllo");
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "source-ranges".to_owned(),
            source.clone(),
        ))
        .expect("session JSON");
        assert_eq!(opened["ok"], true);
        let cord = &opened["view"]["topology"]["cords"][0];
        let range = &cord["source_range"];
        let start_byte = range["start_byte"].as_u64().expect("start byte") as usize;
        let end_byte = range["end_byte"].as_u64().expect("end byte") as usize;
        assert_eq!(
            &source[start_byte..end_byte],
            "greeting.value > output.text"
        );
        assert_eq!(range["source_revision"], 0);
        assert_eq!(range["provenance"], "authored");
        assert!(
            start_byte
                > range["start_utf16"]
                    .as_u64()
                    .expect("browser source offset") as usize,
            "UTF-8 and browser UTF-16 offsets must remain distinct"
        );
        for (field, expected, path) in [
            (
                "from_port_range",
                "value",
                "root/greeting/port/outgoing/value",
            ),
            ("to_port_range", "text", "root/output/port/receiving/text"),
        ] {
            let endpoint = &cord[field];
            let endpoint_start = endpoint["start_byte"].as_u64().unwrap() as usize;
            let endpoint_end = endpoint["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(&source[endpoint_start..endpoint_end], expected);
            assert_eq!(endpoint["provenance"], "authored-endpoint");
            let path_field = if field == "from_port_range" {
                "from_port_path"
            } else {
                "to_port_path"
            };
            assert_eq!(cord[path_field], path);
        }
    }

    #[test]
    fn source_metadata_uses_parser_direction_and_exact_semantic_spans() {
        let source = "panel 0\n\
interface fixture/duplex {\n\
  > value: fixture/text\n\
  result >: fixture/text\n\
  > audio: fixture/audio\n\
  committed >: fixture/text\n\
}\n\
fixture/box{\n\
  worker: fixture/sink\n\
  export > audio = worker.result\n\
  export value < = worker.result\n\
}\n\
output: fixture/source\n\
sink: fixture/sink\n\
output.value > sink.result\n\
# > comment.value and \"string.out >\" are not ports\n";
        let metadata: Value =
            serde_json::from_str(&panel_source_metadata(source.to_owned())).unwrap();
        assert_eq!(metadata["semantic_available"], true);
        let annotations = metadata["annotations"].as_array().unwrap();
        let names = annotations
            .iter()
            .filter(|entry| entry["kind"] == "port-name")
            .map(|entry| {
                let start = entry["start_byte"].as_u64().unwrap() as usize;
                let end = entry["end_byte"].as_u64().unwrap() as usize;
                (
                    &source[start..end],
                    entry["direction"].as_str().unwrap(),
                    entry["semantic_path"].as_str().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert!(names.contains(&(
            "value",
            "receiving",
            "interface/fixture/duplex/port/receiving/value"
        )));
        assert!(names.contains(&(
            "result",
            "outgoing",
            "interface/fixture/duplex/port/outgoing/result"
        )));
        assert!(names.iter().any(|entry| {
            entry.0 == "value" && entry.1 == "outgoing" && entry.2.ends_with("/from/output/value")
        }));
        assert!(names.iter().any(|entry| {
            entry.0 == "result" && entry.1 == "receiving" && entry.2.ends_with("/to/sink/result")
        }));
        assert!(names.iter().any(|entry| {
            entry.0 == "result"
                && entry.1 == "receiving"
                && entry.2.ends_with("/export/audio/target/worker/result")
        }));
        assert!(names.contains(&(
            "value",
            "receiving",
            "definition/fixture/box/port/receiving/value"
        )));
        assert_eq!(
            annotations
                .iter()
                .filter(|entry| entry["kind"] == "port-sigil")
                .count(),
            6
        );
        assert!(annotations.iter().any(|entry| {
            let start = entry["start_byte"].as_u64().unwrap() as usize;
            let end = entry["end_byte"].as_u64().unwrap() as usize;
            entry["kind"] == "port-sigil"
                && &source[start..end] == "<"
                && entry["direction"] == "receiving"
                && entry["semantic_path"] == "definition/fixture/box/port/receiving/value"
        }));
        assert!(!annotations.iter().any(|entry| {
            let start = entry["start_byte"].as_u64().unwrap() as usize;
            source[..start].ends_with("comment.")
        }));

        let malformed: Value = serde_json::from_str(&panel_source_metadata(
            "panel 0\nsource.value > \n".to_owned(),
        ))
        .unwrap();
        assert_eq!(malformed["semantic_available"], false);
        assert_eq!(malformed["annotations"], serde_json::json!([]));
    }

    #[test]
    fn source_metadata_distinguishes_graph_and_expression_operators_by_parse_context() {
        let source = "panel 0\nages: fixture/source\nadults: fixture/sink\n\
                      ages > keep { it > 18 } > adults\n";
        let metadata: Value =
            serde_json::from_str(&panel_source_metadata(source.to_owned())).unwrap();
        assert_eq!(metadata["semantic_available"], true);
        let annotations = metadata["annotations"].as_array().unwrap();

        let graph = annotations
            .iter()
            .filter(|entry| entry["kind"] == "graph-operator")
            .collect::<Vec<_>>();
        let expression = annotations
            .iter()
            .filter(|entry| entry["kind"] == "expression-operator")
            .collect::<Vec<_>>();
        assert_eq!(graph.len(), 2);
        assert_eq!(expression.len(), 1);
        for entry in graph.iter().chain(expression.iter()) {
            let start = entry["start_byte"].as_u64().unwrap() as usize;
            let end = entry["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(&source[start..end], ">");
        }
        assert_ne!(graph[0]["semantic_path"], expression[0]["semantic_path"]);
        assert_eq!(expression[0]["direction"], "greater-than");
    }

    #[test]
    fn wasm_bridge_keeps_source_and_presentation_identities_separate() {
        let moved: Value = serde_json::from_str(&patchbay_move_node(
            SOURCE.to_owned(),
            "greeting".to_owned(),
            16,
            0,
        ))
        .expect("bridge result JSON");
        assert_eq!(moved["ok"], true);
        assert_eq!(moved["source_revision"], 0);
        assert_eq!(moved["presentation_revision"], 1);
        assert_eq!(moved["positions"]["greeting"]["x"], 16);

        let changed: Value = serde_json::from_str(&patchbay_replace_source(
            SOURCE.to_owned(),
            SOURCE.replace("hello", "goodbye"),
        ))
        .expect("bridge result JSON");
        assert_eq!(changed["ok"], true);
        assert_eq!(changed["source_revision"], 1);
        assert_eq!(changed["presentation_revision"], 0);
        assert_ne!(changed["semantic_hash"], moved["semantic_hash"]);

        let explained: Value = serde_json::from_str(&explain_panel(
            "panel 0\n\
             example/upper{\n\
               worker: text/uppercase\n\
               export > text = worker.text\n\
               export uppercased > = worker.text\n\
             }\n\
             source: std/literal { value = \"hello\" }\n\
             transform: example/upper\n\
             sink: display/text\n\
             source.value > transform.text\n\
             transform.uppercased > sink.text\n"
                .to_owned(),
        ))
        .expect("explanation JSON");
        assert_eq!(explained["ok"], true);
        assert!(
            explained["logical"]
                .as_str()
                .is_some_and(|value| value.contains("transform: example/upper"))
        );
        assert!(explained["expanded"].as_str().is_some_and(|value| {
            value.contains("transform.worker: text/uppercase")
                || value.contains("transform.worker: text/uppercase")
        }));
    }

    #[test]
    fn installed_profile_plan_does_not_become_authoritative_placement() {
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "placement-authority".to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("session JSON");

        assert_eq!(opened["ok"], true);
        assert!(opened["view"]["plan"].is_object());
        let logical = opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .expect("semantic nodes");
        assert!(!logical.is_empty());
        assert!(logical.iter().all(|node| node.get("placement").is_none()));
        assert!(logical.iter().all(|node| node["availability"].is_null()));
        assert!(
            logical
                .iter()
                .all(|node| node["contract_identity"].as_str().is_some())
        );
        let planned = opened["view"]["topology"]["planned_realization"]["nodes"]
            .as_array()
            .expect("planned nodes");
        assert!(!planned.is_empty());
        assert!(planned.iter().all(|node| node.get("placement").is_none()));
        assert!(planned.iter().all(|node| {
            node["binding"]["implementation_identity"]
                .as_str()
                .is_some()
                && node["binding"]["artifact_digest"].as_str().is_some()
                && node["binding"]["host_observation_identity"]
                    .as_str()
                    .is_some()
        }));
    }

    #[test]
    fn logical_topology_is_stable_while_pinned_plan_bindings_differ() {
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "plan-variant-projection".to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("session JSON");
        let logical: Vec<conduit_patchbay::PatchbayNodeProjection> =
            serde_json::from_value(opened["view"]["topology"]["logical_nodes"].clone())
                .expect("logical projection");
        let first: conduit_patchbay::PlanSnapshot =
            serde_json::from_value(opened["view"]["plan"].clone()).expect("plan snapshot");
        let mut second = first.clone();
        second.identity = "sha256:alternate-plan".to_owned();
        second.bindings[0].implementation_id = "fixture/alternate-provider".to_owned();
        second.bindings[0].implementation_identity = "sha256:alternate-provider".to_owned();
        second.bindings[0].host_id = "fixture/alternate-host".to_owned();
        second.bindings[0].host_observation_id = "fixture/alternate-observation".to_owned();
        second.bindings[0].host_observation_identity = "sha256:alternate-observation".to_owned();
        second.bindings[0]
            .resources
            .push(conduit_patchbay::PlanResourceBindingProjection {
                binding_id: "fixture/device-binding".to_owned(),
                resource_kind: "device".to_owned(),
                resource_id: "fixture/device-b".to_owned(),
                host_observation_id: "fixture/alternate-observation".to_owned(),
                lease_id: None,
            });

        let first_view = planned_realization_projection(
            &first,
            &logical,
            "candidate",
            None,
            Some(first.identity.clone()),
            Some(&first.source_semantic_hash),
        );
        let second_view = planned_realization_projection(
            &second,
            &logical,
            "candidate",
            None,
            Some(second.identity.clone()),
            Some(&second.source_semantic_hash),
        );

        assert_eq!(first.source_semantic_hash, second.source_semantic_hash);
        assert_eq!(
            first_view.nodes[0].logical_origin,
            second_view.nodes[0].logical_origin
        );
        assert_eq!(first_view.nodes[0].inputs, second_view.nodes[0].inputs);
        assert_ne!(first_view.plan_identity, second_view.plan_identity);
        assert_ne!(
            first_view.nodes[0].binding.implementation_identity,
            second_view.nodes[0].binding.implementation_identity
        );
        assert_ne!(
            first_view.nodes[0].binding.host_observation_identity,
            second_view.nodes[0].binding.host_observation_identity
        );
        assert_eq!(
            second_view.nodes[0].binding.resources[0].resource_id,
            "fixture/device-b"
        );
        // Re-projecting does not consult the current registry or discovery;
        // the alternate pinned facts remain byte-for-byte stable.
        assert_eq!(
            second_view,
            planned_realization_projection(
                &second,
                &logical,
                "candidate",
                None,
                Some(second.identity.clone()),
                Some(&second.source_semantic_hash),
            )
        );
    }

    #[test]
    fn expanded_is_unavailable_without_an_exact_plan() {
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "no-exact-plan".to_owned(),
            "panel 0\nunfinished :".to_owned(),
        ))
        .expect("session JSON");
        assert_eq!(opened["ok"], true, "{opened}");
        assert!(opened["view"]["plan"].is_null());
        assert!(opened["view"]["topology"]["planned_realization"].is_null());
        assert_eq!(
            opened["view"]["topology"]["planned_realization_status"],
            "no-exact-plan"
        );
    }

    #[test]
    fn mismatched_active_plan_fails_closed_without_candidate_blending() {
        let workspace =
            conduit_patchbay::Workspace::new("mismatched/active-plan", SOURCE).expect("workspace");
        let plan = exact_plan_snapshot(SOURCE).expect("exact plan");
        let run = conduit_patchbay::RunSnapshot {
            run_id: "run/mismatched".to_owned(),
            plan_identity: "sha256:not-the-supplied-plan".to_owned(),
            source_semantic_hash: plan.source_semantic_hash.clone(),
            state: conduit_patchbay::RunState::Active,
        };
        let view = authoritative_patchbay_view(&workspace, Some(plan), Some(run), None, &[])
            .expect("bounded projection");

        assert!(view.plan.is_none());
        assert!(view.run.is_none());
        assert!(view.topology.planned_realization.is_none());
        assert_eq!(
            view.topology.planned_realization_status,
            "active-plan-mismatch"
        );
    }

    #[test]
    fn semantic_contract_change_invalidates_the_previous_plan_source_identity() {
        let original = conduit_patchbay::Workspace::new("semantic/original", SOURCE)
            .expect("source workspace");
        let changed_source = SOURCE.replace("std/literal", "std/number");
        let changed = conduit_patchbay::Workspace::new("semantic/changed", &changed_source)
            .expect("changed workspace");
        let original_hash = original
            .semantic()
            .source_semantic_hash
            .expect("original semantic identity");
        let changed_hash = changed
            .semantic()
            .source_semantic_hash
            .expect("changed semantic identity");
        let original_plan = exact_plan_snapshot(SOURCE).expect("original exact plan");

        assert_ne!(original_hash, changed_hash);
        assert_eq!(original_plan.source_semantic_hash, original_hash);
        assert_ne!(original_plan.source_semantic_hash, changed_hash);
    }

    #[test]
    fn persistent_patchbay_session_returns_only_rust_resolved_facts() {
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/session".to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true);
        assert_eq!(
            opened["view"]["protocol_version"],
            conduit_patchbay::PATCHBAY_PROTOCOL_VERSION
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["type_id"],
            "std/text"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["display_label"],
            "value >"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["accessible_label"],
            "value, outgoing port"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["semantic_path"],
            "root/greeting/port/outgoing/value"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][1]["inputs"][0]["display_label"],
            "> text"
        );
        assert_eq!(
            opened["view"]["topology"]["cords"][0]["compatibility"]["compatible"],
            true
        );
        assert!(
            opened["view"]["topology"]["logical_nodes"][0]
                .get("fake_activity")
                .is_none()
        );

        let request = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/session",
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{
                "MoveNode": {
                    "node_id": "greeting",
                    "position": {"x": 12, "y": 24}
                }
            }]
        });
        let moved: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/session".to_owned(),
            request.to_string(),
        ))
        .expect("move JSON");
        assert_eq!(moved["ok"], true);
        assert_eq!(moved["result"]["candidate_revision"]["presentation"], 1);
        assert_eq!(moved["result"]["disposition"], "committed");
        assert_eq!(
            moved["result"]["compatibility"]["code"],
            "CND-PBY-PRESENTATION-ONLY"
        );

        let stale: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/session".to_owned(),
            request.to_string(),
        ))
        .expect("stale JSON");
        assert_eq!(stale["ok"], false);
        assert_eq!(stale["code"], "CND-PBY-003");
        assert_eq!(stale["disposition"], "rejected");

        let observed: Value =
            serde_json::from_str(&patchbay_session_view("test/session".to_owned()))
                .expect("view JSON");
        assert_eq!(
            observed["view"]["presentation"]["node_positions"]["greeting"]["x"],
            12
        );

        let replacement = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/session",
            "expected_source_revision": 0,
            "expected_presentation_revision": 1,
            "operations": [{
                "ReplaceSource": {"source": SOURCE.replace("hello", "candidate")}
            }]
        });
        let replaced: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/session".to_owned(),
            replacement.to_string(),
        ))
        .expect("replacement JSON");
        assert_eq!(replaced["ok"], true, "{replaced}");
        assert_eq!(
            replaced["result"]["compatibility"]["code"],
            "CND-PBY-EXACT-PLAN"
        );
        assert_eq!(
            replaced["result"]["compatibility"]["plan_disposition"],
            "candidate-only"
        );
        assert!(
            replaced["result"]["compatibility"]["candidate_plan_identity"]
                .as_str()
                .is_some_and(|identity| identity.starts_with("sha256:"))
        );
        assert_eq!(replaced["result"]["candidate_revision"]["source"], 1);
    }

    #[test]
    fn presentation_lenses_are_rust_owned_and_keep_semantic_runtime_layers_separate() {
        let session = "test/presentation-lenses";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["view"]["presentation"]["mode"], "build");
        assert_eq!(opened["view"]["presentation"]["lens"], "face");
        assert_eq!(
            opened["view"]["presentation"]["opening_reason"],
            "no-usable-task-front-declared"
        );
        assert_eq!(
            opened["view"]["topology"]["cords"][0]["owner_kind"],
            "enclosing-panel"
        );
        assert_eq!(
            opened["view"]["topology"]["cords"][0]["boundary_rule"],
            "exported-public-ports-only"
        );
        let source_identity = opened["view"]["source"]["identity"].clone();
        let semantic_identity = opened["view"]["semantic"]["source_semantic_hash"].clone();
        let plan_identity = opened["view"]["plan"]["identity"].clone();
        let cord_path = opened["view"]["topology"]["cords"][0]["semantic_path"]
            .as_str()
            .expect("cord path");
        let request = serde_json::json!({
            "protocol_version": 0,
            "document_id": session,
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [
                {"Navigate": {
                    "mode": "inspect",
                    "lens": "context",
                    "topology": "expanded"
                }},
                {"SelectSubject": {"subject": {
                    "kind": "cord",
                    "path": cord_path
                }}}
            ]
        });
        let changed: Value = serde_json::from_str(&patchbay_apply_transaction(
            session.to_owned(),
            request.to_string(),
        ))
        .expect("navigation JSON");
        assert_eq!(changed["ok"], true, "{changed}");
        assert_eq!(changed["view"]["presentation"]["mode"], "inspect");
        assert_eq!(changed["view"]["presentation"]["lens"], "context");
        assert_eq!(changed["view"]["presentation"]["topology"], "expanded");
        assert_eq!(changed["view"]["source"]["identity"], source_identity);
        assert_eq!(
            changed["view"]["semantic"]["source_semantic_hash"],
            semantic_identity
        );
        assert_eq!(changed["view"]["plan"]["identity"], plan_identity);
        let layers = changed["view"]["configuration_layers"]
            .as_array()
            .expect("configuration layers");
        for owner in [
            "panel-instance",
            "upstream-semantic-cords",
            "exact-plan",
            "presentation-document",
        ] {
            assert!(
                layers.iter().any(|layer| layer["owner"] == owner),
                "projects {owner} separately"
            );
        }
        let authored = layers
            .iter()
            .find(|layer| layer["owner"] == "panel-instance")
            .expect("instance-authored layer");
        assert_eq!(authored["persistence"], "source-document");
        assert_eq!(authored["activation"], "re-resolution-or-plan-transition");
    }

    #[test]
    fn unloaded_at_rest_wasm_projection_has_no_plan_run_authority_or_effect() {
        let inspected: Value = serde_json::from_str(&patchbay_inspect_at_rest(
            "shelf/hello".to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("at-rest JSON");
        assert_eq!(inspected["ok"], true, "{inspected}");
        let projection = &inspected["inspection"];
        assert_eq!(projection["presentation"]["lens"], "at-rest");
        assert_eq!(
            projection["definition"]["provider_availability"],
            "not-observed"
        );
        assert_eq!(projection["definition"]["operations"]["resolved"], false);
        assert_eq!(
            projection["definition"]["operations"]["authority_acquired"],
            false
        );
        assert_eq!(projection.get("plan"), None);
        assert_eq!(projection.get("run"), None);
        assert_eq!(projection.get("evidence"), None);
    }

    #[test]
    fn two_definition_instances_keep_config_external_cords_and_internal_owner_distinct() {
        let source = r#"panel 0
example/upper-box(value: std/text = "default") {
  source: std/literal { value = "inside" }
  upper: text/uppercase
  source.value > upper.text
  export value > = upper.text
  bind value = source.value
}
first: example/upper-box { value = "first" }
second: example/upper-box { value = "second" }
sink_one: display/text
sink_two: display/text
first.value > sink_one.text
second.value > sink_two.text
"#;
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/two-composite-instances".to_owned(),
            source.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");
        let nodes = opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .expect("logical nodes");
        let first = nodes.iter().find(|node| node["id"] == "first").unwrap();
        let second = nodes.iter().find(|node| node["id"] == "second").unwrap();
        assert_eq!(first["config"]["value"]["display_value"], "first");
        assert_eq!(second["config"]["value"]["display_value"], "second");
        let cords = opened["view"]["topology"]["cords"]
            .as_array()
            .expect("external cords");
        assert_eq!(cords.len(), 2);
        assert!(
            cords
                .iter()
                .all(|cord| cord["owner_kind"] == "enclosing-panel")
        );
        let composites = opened["view"]["topology"]["composites"]
            .as_array()
            .expect("composite instances");
        assert_eq!(composites.len(), 2, "{opened}");
        assert!(composites.iter().all(|composite| {
            composite["definition"] == "example/upper-box"
                && composite["internal_cords"][0]["owner_kind"] == "panel-definition"
        }));
    }

    #[test]
    fn browser_exact_run_stays_pinned_while_a_candidate_source_revision_changes() {
        let session_id = "test/browser-active-run";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true);

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        assert_eq!(started["state"], "active");
        assert_eq!(started["view"]["run"]["state"], "Active");
        let binding = test_run_binding(&started);
        let active_plan_source = started["source_semantic_hash"].clone();

        let evidence: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_evidence,
            session_id,
            binding,
            0,
            1
        ))
        .expect("evidence JSON");
        assert_eq!(evidence["ok"], true, "{evidence}");
        assert_eq!(evidence["status"]["kind"], "available");
        assert!(
            evidence["records"]
                .as_array()
                .is_some_and(|records| records.len() <= 1)
        );
        let repeated: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_evidence,
            session_id,
            binding,
            0,
            1
        ))
        .expect("repeated evidence JSON");
        assert_eq!(repeated, evidence);

        let malformed_wake: Value = serde_json::from_str(&bound_run!(
            patchbay_notify_host_operation,
            session_id,
            binding,
            "Not an exact host operation".to_owned(),
        ))
        .expect("malformed wake JSON");
        assert_eq!(malformed_wake["ok"], false);
        assert_eq!(malformed_wake["code"], "CND-PBY-012");

        let unrelated_wake: Value = serde_json::from_str(&bound_run!(
            patchbay_notify_host_operation,
            session_id,
            binding,
            "conduit/unrelated-host-operation".to_owned(),
        ))
        .expect("unrelated wake JSON");
        assert_eq!(unrelated_wake["ok"], true, "{unrelated_wake}");
        assert_eq!(unrelated_wake["state"], "active");

        let replacement = serde_json::json!({
            "protocol_version": 0,
            "document_id": session_id,
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{
                "ReplaceSource": {"source": "panel 0\nbroken :"}
            }]
        });
        let edited: Value = serde_json::from_str(&patchbay_apply_transaction(
            session_id.to_owned(),
            replacement.to_string(),
        ))
        .expect("candidate edit JSON");
        assert_eq!(edited["ok"], true, "{edited}");
        assert_eq!(edited["result"]["compatibility"]["compatible"], false);
        assert_eq!(
            edited["result"]["compatibility"]["plan_disposition"],
            "unavailable"
        );

        let view: Value = serde_json::from_str(&patchbay_session_view(session_id.to_owned()))
            .expect("active view JSON");
        assert_eq!(view["ok"], true, "{view}");
        assert_eq!(
            view["view"]["run"]["source_semantic_hash"],
            active_plan_source
        );
        assert!(view["view"]["source"].get("semantic_hash").is_none());
        assert_eq!(view["view"]["run"]["state"], "Active");

        let stale: Value = serde_json::from_str(&patchbay_pump_exact_run(
            session_id.to_owned(),
            binding.run_id.clone(),
            binding.source_revision + 1,
            binding.plan_identity.clone(),
            1,
        ))
        .expect("stale control JSON");
        assert_eq!(stale["ok"], false, "{stale}");
        assert_eq!(stale["code"], "CND-PBY-016");

        let snapshot: Value = serde_json::from_str(&bound_run!(
            patchbay_snapshot_exact_run,
            session_id,
            binding
        ))
        .expect("recovery snapshot JSON");
        assert_eq!(snapshot["ok"], true, "{snapshot}");
        assert_eq!(snapshot["state"], "active");
        assert_eq!(snapshot["run_id"], binding.run_id);

        let cancelled: Value = serde_json::from_str(&bound_run!(
            patchbay_cancel_exact_run,
            session_id,
            binding,
            "abort".to_owned(),
        ))
        .expect("cancel JSON");
        assert_eq!(cancelled["ok"], true, "{cancelled}");
        assert_eq!(cancelled["state"], "cancelled");
        assert_eq!(cancelled["view"]["run"]["state"], "Terminal");

        let terminal_evidence: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_evidence,
            session_id,
            binding,
            0,
            1
        ))
        .expect("terminal evidence JSON");
        assert_eq!(terminal_evidence["ok"], true, "{terminal_evidence}");
        assert_eq!(terminal_evidence["status"]["kind"], "available");
        assert_eq!(
            terminal_evidence["records"].as_array().map(Vec::len),
            Some(1)
        );

        let disposed: Value =
            serde_json::from_str(&bound_run!(patchbay_dispose_exact_run, session_id, binding))
                .expect("dispose JSON");
        assert_eq!(disposed["ok"], true, "{disposed}");
        assert!(disposed["view"]["run"].is_null(), "{disposed}");
    }

    #[test]
    fn active_and_candidate_plans_are_never_joined() {
        let session_id = "test/separate-active-candidate-plans";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");
        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        let active_plan = started["plan_identity"].as_str().unwrap().to_owned();
        let active_bindings = started["view"]["plan"]["bindings"].clone();

        let replacement = serde_json::json!({
            "protocol_version": 0,
            "document_id": session_id,
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{
                "ReplaceSource": {"source": SOURCE.replace("hello", "candidate")}
            }]
        });
        let edited: Value = serde_json::from_str(&patchbay_apply_transaction(
            session_id.to_owned(),
            replacement.to_string(),
        ))
        .expect("candidate edit JSON");
        assert_eq!(edited["ok"], true, "{edited}");

        let observed: Value =
            serde_json::from_str(&patchbay_session_view(session_id.to_owned())).expect("view JSON");
        let view = &observed["view"];
        let realization = &view["topology"]["planned_realization"];
        assert_eq!(view["plan"]["identity"], active_plan);
        assert_eq!(view["run"]["plan_identity"], active_plan);
        assert_eq!(realization["selection"], "active-run");
        assert_eq!(realization["plan_identity"], active_plan);
        assert_eq!(realization["current_source_matches"], false);
        assert_ne!(realization["candidate_plan_identity"], active_plan);
        assert_eq!(view["plan"]["bindings"], active_bindings);
        assert!(
            realization["notice"]
                .as_str()
                .is_some_and(|notice| notice.contains("remains separate"))
        );
    }

    #[test]
    fn browser_learned_promotion_fails_closed_without_host_policy_and_backend() {
        let session_id = "test/browser-learned-promotion";
        let source = include_str!("../../../examples/learned-lifecycle.panel");
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            source.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], false, "{started}");
        assert_eq!(started["code"], "CND-IMP-001", "{started}");
    }

    #[test]
    fn browser_run_identity_is_valid_bounded_and_revision_specific() {
        let first = browser_exact_run_id("tour/worker-run/1", 0);
        let second = browser_exact_run_id("tour/worker-run/1", 1);
        assert_eq!(first.len(), 36);
        assert!(conduit_core::Id::new(&first).is_ok());
        assert_ne!(first, second);
    }

    #[test]
    fn browser_patchbay_session_capacity_is_finite() {
        for index in 0..super::MAXIMUM_PATCHBAY_SESSIONS {
            let opened: Value = serde_json::from_str(&patchbay_open_session(
                format!("test/browser-session-capacity/{index}"),
                SOURCE.to_owned(),
            ))
            .expect("session capacity JSON");
            assert_eq!(opened["ok"], true, "{opened}");
        }
        let rejected: Value = serde_json::from_str(&patchbay_open_session(
            "test/browser-session-capacity/overflow".to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("session overflow JSON");
        assert_eq!(rejected["ok"], false, "{rejected}");
        assert_eq!(rejected["code"], "CND-PBY-006");
    }

    #[test]
    fn browser_exact_run_exposes_one_public_latest_value_watch_without_identity_drift() {
        let session_id = "test/browser-latest-watch";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        let binding = test_run_binding(&started);
        let plan_identity = started["plan_identity"].clone();
        let source_identity = started["source_semantic_hash"].clone();
        let admission = &started["view"]["plan"]["watch_admissions"][0];
        assert_eq!(admission["retention"], "latest");
        assert_eq!(admission["maximum_history"], 1);
        assert_eq!(admission["sensitivity_ceiling"], "public");
        let watch_id = admission["id"].as_str().expect("Watch identity");

        let wrong_operator: Value = serde_json::from_str(&patchbay_attach_exact_watch(
            session_id.to_owned(),
            binding.run_id.clone(),
            binding.source_revision,
            binding.plan_identity.clone(),
            "operator/wrong".to_owned(),
            watch_id.to_owned(),
        ))
        .expect("operator rejection JSON");
        assert_eq!(wrong_operator["ok"], false, "{wrong_operator}");
        assert_eq!(wrong_operator["code"], "CND-WAT-004");

        let capacity_rejected: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            "watch/not-admitted".to_owned(),
        ))
        .expect("capacity rejection JSON");
        assert_eq!(capacity_rejected["ok"], false, "{capacity_rejected}");
        assert_eq!(capacity_rejected["code"], "CND-WAT-002");

        let attached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("attach JSON");
        assert_eq!(attached["ok"], true, "{attached}");
        assert_eq!(attached["plan_identity"], plan_identity);
        assert_eq!(attached["source_semantic_hash"], source_identity);
        assert_eq!(attached["usage"]["attached_slots"], 1);

        let detached: Value = serde_json::from_str(&bound_run!(
            patchbay_detach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("detach JSON");
        assert_eq!(detached["ok"], true, "{detached}");
        assert_eq!(detached["plan_identity"], plan_identity);
        assert_eq!(detached["usage"]["attached_slots"], 0);
        let reattached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("reattach JSON");
        assert_eq!(reattached["ok"], true, "{reattached}");

        let mut pumped: Value = serde_json::json!({"state": "active"});
        for _ in 0..8 {
            pumped = serde_json::from_str(&bound_run!(
                patchbay_pump_exact_run,
                session_id,
                binding,
                64
            ))
            .expect("pump JSON");
            assert_eq!(pumped["ok"], true, "{pumped}");
            if pumped["state"] != "active" {
                break;
            }
        }
        assert_eq!(pumped["state"], "succeeded", "{pumped}");

        let watched: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
            0,
            1,
        ))
        .expect("Watch JSON");
        assert_eq!(watched["ok"], true, "{watched}");
        assert_eq!(watched["plan_identity"], plan_identity);
        assert_eq!(watched["source_semantic_hash"], source_identity);
        assert_eq!(watched["status"]["kind"], "available");
        assert_eq!(watched["records"].as_array().map(Vec::len), Some(1));
        assert_eq!(watched["records"][0]["material"]["kind"], "preview");
        assert_eq!(watched["records"][0]["material"]["text"], "hello\n");
        assert_eq!(
            watched["records"][0]["material"]["renderer"]["id"],
            "conduit.browser/utf8-text"
        );
        assert_eq!(
            watched["records"][0]["producing_host"],
            "conduit/browser-worker"
        );
        assert_eq!(watched["records"][0]["time_basis"], "clock/browser-worker");
        assert_eq!(watched["records"][0]["clock_uncertainty_ticks"], 0);
        assert_eq!(watched["records"][0]["truncated"], false);
        assert!(
            watched["records"][0]["content_hash"]
                .as_str()
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
    }

    #[test]
    fn browser_exact_run_keeps_one_public_text_ticker_watch_live_and_bounded() {
        const TICKER_SOURCE: &str = r#"panel 0

clock: time/ticker {
    duration_ticks = 10
    time_basis = ref("conduit.clock/monotonic-ticks")
    maximum_pending = 1
}
drain: flow/discard

clock.tick > drain.item {
    capacity = 1
    max_value_bytes = 32
    max_queued_bytes = 32
    low_watermark = 0
    high_watermark = 1
    pressure = block
}
"#;

        let session_id = "test/browser-live-ticker-watch";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            TICKER_SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        let binding = test_run_binding(&started);
        let run_id = started["run_id"].clone();
        let plan_identity = started["plan_identity"].clone();
        let source_identity = started["source_semantic_hash"].clone();
        let admission = &started["view"]["plan"]["watch_admissions"][0];
        assert_eq!(admission["retention"], "latest");
        assert_eq!(admission["representation_id"], "std/text");
        assert_eq!(admission["sensitivity_ceiling"], "public");
        let watch_id = admission["id"].as_str().expect("Watch identity");

        let attached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("attach JSON");
        assert_eq!(attached["ok"], true, "{attached}");

        // A deliberately slow Patchbay reader does not delay the actual cord.
        // Latest-only retention replaces isolated preview storage and reports
        // one deterministic cursor gap when the reader catches up.
        for expected_tick in 0..8_u64 {
            let pumped: Value = serde_json::from_str(&bound_run!(
                patchbay_pump_exact_run,
                session_id,
                binding,
                256
            ))
            .expect("pump JSON");
            assert_eq!(pumped["ok"], true, "{pumped}");
            assert_eq!(pumped["state"], "waiting", "{pumped}");
            assert!(pumped["terminal"].is_null(), "{pumped}");
            assert_eq!(pumped["run_id"], run_id);
            assert_eq!(pumped["plan_identity"], plan_identity);
            assert_eq!(pumped["source_semantic_hash"], source_identity);
            assert_eq!(pumped["value_storage"]["resident_slots"], 0);
            assert_eq!(pumped["value_storage"]["resident_bytes"], 0);
            assert!(
                pumped["value_storage"]["high_water_slots"]
                    .as_u64()
                    .is_some_and(|slots| slots <= 1),
                "{pumped}"
            );
            assert!(
                pumped["value_storage"]["high_water_bytes"]
                    .as_u64()
                    .is_some_and(|bytes| bytes <= 32),
                "{pumped}"
            );

            let deadline = pumped["next_timer_deadline"]
                .as_u64()
                .expect("pending exact timer deadline");
            assert_eq!(deadline, (expected_tick + 1) * 11);
            let advanced: Value = serde_json::from_str(&bound_run!(
                patchbay_advance_exact_run,
                session_id,
                binding,
                deadline
            ))
            .expect("advance JSON");
            assert_eq!(advanced["ok"], true, "{advanced}");
            assert_eq!(advanced["run_id"], run_id);
        }

        let caught_up: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
            0,
            1,
        ))
        .expect("slow Watch read JSON");
        assert_eq!(caught_up["ok"], true, "{caught_up}");
        assert_eq!(caught_up["status"]["kind"], "gap", "{caught_up}");
        assert_eq!(caught_up["records"][0]["material"]["text"], "7\n");
        let mut cursor = caught_up["next_cursor"]
            .as_u64()
            .expect("caught-up Watch cursor");

        // The executor is Waiting here. Instrument control remains legal and
        // detaching preserves the already copied latest preview.
        let detached: Value = serde_json::from_str(&bound_run!(
            patchbay_detach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("waiting detach JSON");
        assert_eq!(detached["ok"], true, "{detached}");
        assert_eq!(detached["usage"]["retained_observations"], 1);
        let reattached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("waiting reattach JSON");
        assert_eq!(reattached["ok"], true, "{reattached}");

        for expected_tick in 8..80_u64 {
            let pumped: Value = serde_json::from_str(&bound_run!(
                patchbay_pump_exact_run,
                session_id,
                binding,
                256
            ))
            .expect("pump JSON");
            assert_eq!(pumped["ok"], true, "{pumped}");
            assert_eq!(pumped["state"], "waiting", "{pumped}");

            let watched: Value = serde_json::from_str(&bound_run!(
                patchbay_read_exact_watch,
                session_id,
                binding,
                watch_id.to_owned(),
                cursor,
                1,
            ))
            .expect("Watch JSON");
            assert_eq!(watched["ok"], true, "{watched}");
            assert_eq!(watched["status"]["kind"], "available", "{watched}");
            assert_eq!(
                watched["records"][0]["material"]["text"],
                format!("{expected_tick}\n")
            );
            cursor = watched["next_cursor"].as_u64().expect("next Watch cursor");

            let deadline = pumped["next_timer_deadline"]
                .as_u64()
                .expect("pending exact timer deadline");
            assert_eq!(deadline, (expected_tick + 1) * 11);
            let advanced: Value = serde_json::from_str(&bound_run!(
                patchbay_advance_exact_run,
                session_id,
                binding,
                deadline
            ))
            .expect("advance JSON");
            assert_eq!(advanced["ok"], true, "{advanced}");
        }

        let cancelled: Value = serde_json::from_str(&bound_run!(
            patchbay_cancel_exact_run,
            session_id,
            binding,
            "abort".to_owned(),
        ))
        .expect("cancel JSON");
        assert_eq!(cancelled["ok"], true, "{cancelled}");
        assert_eq!(cancelled["state"], "cancelled");
        assert_eq!(cancelled["run_id"], run_id);
        assert_eq!(cancelled["evidence_store"]["maximum_events"], 256);
        assert_eq!(cancelled["evidence_store"]["maximum_bytes"], 262_144);
        assert!(
            cancelled["evidence_store"]["retained_events"]
                .as_u64()
                .is_some_and(|events| events <= 256),
            "{cancelled}"
        );
        assert!(
            cancelled["evidence_store"]["retained_bytes"]
                .as_u64()
                .is_some_and(|bytes| bytes <= 262_144),
            "{cancelled}"
        );
        assert!(
            cancelled["evidence_store"]["dropped_events"]
                .as_u64()
                .is_some_and(|events| events > 0),
            "{cancelled}"
        );
        assert!(
            cancelled["evidence"]
                .as_array()
                .is_some_and(|events| events.len() <= 32),
            "Patchbay projection exceeded its separate presentation bound: {cancelled}"
        );

        let gap: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_evidence,
            session_id,
            binding,
            0,
            32
        ))
        .expect("evidence gap JSON");
        assert_eq!(gap["ok"], true, "{gap}");
        assert_eq!(gap["status"]["kind"], "gap", "{gap}");
        let resume_at = gap["status"]["resume_at"]
            .as_u64()
            .expect("rolling evidence gap resume cursor");
        assert!(resume_at > 0, "{gap}");

        let resumed: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_evidence,
            session_id,
            binding,
            resume_at,
            256,
        ))
        .expect("resumed evidence JSON");
        assert_eq!(resumed["ok"], true, "{resumed}");
        assert_eq!(resumed["status"]["kind"], "available", "{resumed}");
        assert!(
            resumed["records"].as_array().is_some_and(|records| records
                .iter()
                .any(|record| record["event_kind"] == "terminal")),
            "terminal evidence was not retained after finalization: {resumed}"
        );
    }

    #[test]
    fn living_instrument_runs_the_domain_signal_graph_until_explicit_stop() {
        const SOURCE: &str = include_str!("../../../examples/living-instrument.panel");
        let session_id = "test/living-instrument";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        let binding = test_run_binding(&started);
        let contracts = started["view"]["topology"]["logical_nodes"]
            .as_array()
            .expect("planned nodes")
            .iter()
            .filter_map(|node| node["contract_id"].as_str())
            .collect::<BTreeSet<_>>();
        for required in [
            "conduit.media/event/from-ticker",
            "conduit.media/event/tee",
            "conduit.media/control/clock-divider",
            "conduit.media/control/sequencer",
            "conduit.media/control/slew",
            "conduit.media/control/merge",
            "conduit.media/control/mixer",
            "conduit.media/control/register",
            "conduit.media/control/scope",
        ] {
            assert!(contracts.contains(required), "instrument plans {required}");
        }
        let scope_cord = started["view"]["topology"]["cords"]
            .as_array()
            .expect("projected cords")
            .iter()
            .find(|cord| cord["from_node"] == "scope" && cord["from_port"] == "text")
            .expect("scope output cord")["id"]
            .as_str()
            .expect("scope cord identity");
        let admission = started["view"]["plan"]["watch_admissions"]
            .as_array()
            .expect("Watch admissions")
            .iter()
            .find(|watch| watch["cord"] == scope_cord)
            .expect("scope owns a public text Watch admission");
        let watch_id = admission["id"].as_str().expect("Watch identity");
        let attached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("attach JSON");
        assert_eq!(attached["ok"], true, "{attached}");

        let mut pumped = Value::Null;
        for _ in 0..8 {
            pumped = serde_json::from_str(&bound_run!(
                patchbay_pump_exact_run,
                session_id,
                binding,
                32,
            ))
            .expect("pump JSON");
            assert_eq!(pumped["ok"], true, "{pumped}");
            if pumped["state"] != "active" {
                break;
            }
        }
        assert_eq!(pumped["state"], "waiting", "{pumped}");
        assert!(pumped["terminal"].is_null(), "{pumped}");
        assert!(pumped["next_timer_deadline"].is_u64(), "{pumped}");

        let watched: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
            0,
            1,
        ))
        .expect("Watch JSON");
        assert_eq!(watched["ok"], true, "{watched}");
        assert_eq!(
            watched["records"][0]["material"]["text"],
            "tick=0 level=128\n"
        );
        let mut cursor = watched["next_cursor"].as_u64().expect("Watch cursor");
        let mut last_watch_tick = 0_u64;
        for _ in 0..6 {
            let deadline = pumped["next_timer_deadline"]
                .as_u64()
                .expect("next instrument deadline");
            let advanced: Value = serde_json::from_str(&bound_run!(
                patchbay_advance_exact_run,
                session_id,
                binding,
                deadline,
            ))
            .expect("advance JSON");
            assert_eq!(advanced["ok"], true, "{advanced}");
            for _ in 0..8 {
                pumped = serde_json::from_str(&bound_run!(
                    patchbay_pump_exact_run,
                    session_id,
                    binding,
                    32,
                ))
                .expect("pump JSON");
                assert_eq!(pumped["ok"], true, "{pumped}");
                if pumped["state"] != "active" {
                    break;
                }
            }
            assert_eq!(pumped["state"], "waiting", "{pumped}");
            let watched: Value = serde_json::from_str(&bound_run!(
                patchbay_read_exact_watch,
                session_id,
                binding,
                watch_id.to_owned(),
                cursor,
                1,
            ))
            .expect("Watch JSON");
            assert_eq!(watched["ok"], true, "{watched}");
            if let Some(material) = watched["records"]
                .as_array()
                .and_then(|records| records.first())
                .and_then(|record| record["material"]["text"].as_str())
            {
                let tick = material
                    .strip_prefix("tick=")
                    .and_then(|value| value.split_once(' '))
                    .and_then(|(tick, _)| tick.parse::<u64>().ok())
                    .expect("instrument Watch tick");
                assert!(tick > last_watch_tick, "{material}");
                last_watch_tick = tick;
            }
            cursor = watched["next_cursor"].as_u64().expect("next Watch cursor");
        }
        assert!(
            last_watch_tick > 2,
            "instrument kept producing: {last_watch_tick}"
        );

        let cancelled: Value = serde_json::from_str(&bound_run!(
            patchbay_cancel_exact_run,
            session_id,
            binding,
            "abort".to_owned(),
        ))
        .expect("cancel JSON");
        assert_eq!(cancelled["ok"], true, "{cancelled}");
        assert_eq!(cancelled["state"], "cancelled", "{cancelled}");
    }

    #[test]
    fn standing_audio_patch_is_waiting_observable_and_explicitly_cancelled() {
        const SOURCE: &str = include_str!("../../../examples/audio-standing-patch.panel");
        let session_id = "test/audio-standing-patch";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            session_id.to_owned(),
            SOURCE.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");

        let started: Value = serde_json::from_str(&patchbay_start_exact_run(session_id.to_owned()))
            .expect("start JSON");
        assert_eq!(started["ok"], true, "{started}");
        let binding = test_run_binding(&started);
        let contracts = started["view"]["topology"]["logical_nodes"]
            .as_array()
            .expect("planned nodes")
            .iter()
            .filter_map(|node| node["contract_id"].as_str())
            .collect::<BTreeSet<_>>();
        for required in [
            "time/ticker",
            "conduit.media/control/sequencer",
            "conduit.media/control/slew",
            "conduit.media/control/register",
            "conduit.media/audio/from-control",
            "conduit.media/audio/tee",
            "conduit.media/audio/mix",
            "conduit.media/audio/gain",
            "conduit.media/audio/resample",
            "conduit.media/audio/meter",
        ] {
            assert!(contracts.contains(required), "audio patch plans {required}");
        }
        let meter_cord = started["view"]["topology"]["cords"]
            .as_array()
            .expect("projected cords")
            .iter()
            .find(|cord| cord["from_node"] == "meter" && cord["from_port"] == "level")
            .expect("meter side-output cord")["id"]
            .as_str()
            .expect("meter cord identity");
        let admission = started["view"]["plan"]["watch_admissions"]
            .as_array()
            .expect("Watch admissions")
            .iter()
            .find(|watch| watch["cord"] == meter_cord)
            .expect("meter owns a bounded public Watch admission");
        let watch_id = admission["id"].as_str().expect("Watch identity");
        let attached: Value = serde_json::from_str(&bound_run!(
            patchbay_attach_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
        ))
        .expect("attach JSON");
        assert_eq!(attached["ok"], true, "{attached}");

        let mut pumped = Value::Null;
        for _ in 0..8 {
            pumped = serde_json::from_str(&bound_run!(
                patchbay_pump_exact_run,
                session_id,
                binding,
                256,
            ))
            .expect("pump JSON");
            assert_eq!(pumped["ok"], true, "{pumped}");
            if pumped["state"] == "waiting" && pumped["next_timer_deadline"].is_u64() {
                break;
            }
        }
        assert_eq!(pumped["state"], "waiting", "{pumped}");
        assert!(pumped["terminal"].is_null(), "{pumped}");
        assert!(pumped["next_timer_deadline"].is_u64(), "{pumped}");

        let watched: Value = serde_json::from_str(&bound_run!(
            patchbay_read_exact_watch,
            session_id,
            binding,
            watch_id.to_owned(),
            0,
            1,
        ))
        .expect("Watch JSON");
        assert_eq!(watched["ok"], true, "{watched}");
        let frame = watched["records"][0]["material"]["text"]
            .as_str()
            .expect("meter reading");
        assert!(frame.starts_with("audio-meter start=0 frames=4 "));
        let cancelled: Value = serde_json::from_str(&bound_run!(
            patchbay_cancel_exact_run,
            session_id,
            binding,
            "abort".to_owned(),
        ))
        .expect("cancel JSON");
        assert_eq!(cancelled["ok"], true, "{cancelled}");
        assert_eq!(cancelled["state"], "cancelled", "{cancelled}");
    }

    #[test]
    fn candidate_connection_rejects_hidden_composite_members() {
        let composite = "panel 0\n\
example/box{\n\
  worker: text/uppercase\n\
  export > text = worker.text\n\
  export uppercased > = worker.text\n\
}\n\
source: std/literal { value = \"hello\" }\n\
box: example/box\n\
sink: display/text\n\
source.value > box.text\n\
box.uppercased > sink.text\n";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/composite".to_owned(),
            composite.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true);
        assert_eq!(
            opened["view"]["topology"]["composites"][0]["exports"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            opened["view"]["topology"]["planned_realization"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| {
                    node["instance"].as_str().is_some_and(|instance| {
                        instance.ends_with("box/worker") || instance.ends_with("box.worker")
                    }) && node["logical_origin"] == "box"
                })
        );
        assert!(
            opened["view"]["topology"]["logical_nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["id"] == "box")
                .is_some_and(|node| {
                    node["inputs"].as_array().unwrap().len() == 1
                        && node["outputs"].as_array().unwrap().len() == 1
                })
        );
        let request = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/composite",
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{
                "Connect": {
                    "from_node": "source",
                    "from_port": "value",
                    "to_node": "box.worker",
                    "to_port": "text",
                    "bounds": {
                        "capacity_items": 1,
                        "max_value_bytes": 64,
                        "max_queued_bytes": 64,
                        "low_watermark_items": 0,
                        "high_watermark_items": 1,
                        "pressure": "block"
                    }
                }
            }]
        });
        let rejected: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/composite".to_owned(),
            request.to_string(),
        ))
        .expect("rejection JSON");
        assert_eq!(rejected["ok"], false);
        assert_eq!(rejected["code"], "CND-PBY-005");
    }

    #[test]
    fn patchbay_projection_reports_deterministic_truncation() {
        let mut source = String::from("panel 0\n");
        for index in 0..513 {
            source.push_str(&format!(
                "literal_{index}: std/literal {{ value = \"{index}\" }}\n\
                 output_{index}: display/text\n\
                 literal_{index}.value > output_{index}.text\n"
            ));
        }
        let opened: Value =
            serde_json::from_str(&patchbay_open_session("test/truncation".to_owned(), source))
                .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");
        assert_eq!(opened["view"]["truncated"], true);
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"]
                .as_array()
                .unwrap()
                .len(),
            1_024
        );
    }

    #[test]
    fn invalid_direction_lesson_projects_authored_graph_without_a_plan() {
        let source = "panel 0\n\
first: std/literal { value = \"First.\\n\" }\n\
second: std/literal { value = \"Second.\\n\" }\n\
first.value > second.value {\n\
  capacity = 1\n\
  max_value_bytes = 1024\n\
  max_queued_bytes = 1024\n\
  low_watermark = 0\n\
  high_watermark = 1\n\
  pressure = block\n\
}\n";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/invalid-direction".to_owned(),
            source.to_owned(),
        ))
        .expect("open JSON");
        assert_eq!(opened["ok"], true, "{opened}");
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            opened["view"]["topology"]["cords"][0]["validity"],
            "wrong-direction"
        );
        assert!(
            opened["view"]["topology"]["cords"][0]["to_anchor"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][1]["outputs"][0]["id"],
            "value"
        );
        assert_eq!(opened["view"]["diagnostics"][0]["code"], "CND-CMP-003");
        assert!(
            opened["view"]["diagnostics"][0]["explanation"]
                .as_str()
                .unwrap()
                .contains("Outgoing port used as destination")
        );
        assert!(opened["view"].get("plan").is_none());
        assert!(opened["view"].get("run").is_none());
        assert!(
            opened["view"]
                .get("evidence")
                .is_none_or(|evidence| evidence.as_array().is_some_and(Vec::is_empty)),
            "a check diagnostic is not execution evidence"
        );
    }

    #[test]
    fn incomplete_source_commits_current_revision_and_preserves_layout() {
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/incomplete-edit".to_owned(),
            SOURCE.to_owned(),
        ))
        .unwrap();
        assert_eq!(opened["ok"], true);
        let moved = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/incomplete-edit",
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{"MoveNode": {
                "node_id": "greeting",
                "position": {"x": 77, "y": 88}
            }}]
        });
        let moved: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/incomplete-edit".to_owned(),
            moved.to_string(),
        ))
        .unwrap();
        assert_eq!(moved["ok"], true);
        let incomplete = "panel 0\ngreeting: std/literal {\n value =\npreserved :";
        let replacement = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/incomplete-edit",
            "expected_source_revision": 0,
            "expected_presentation_revision": 1,
            "operations": [{"ReplaceSource": {"source": incomplete}}]
        });
        let edited: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/incomplete-edit".to_owned(),
            replacement.to_string(),
        ))
        .unwrap();
        assert_eq!(edited["ok"], true, "{edited}");
        assert_eq!(edited["result"]["source"]["revision"], 1);
        assert!(edited["result"]["source"].get("semantic_hash").is_none());
        assert!(edited["result"]["source"]["identity"].as_str().is_some());
        assert_eq!(edited["view"]["topology"]["source_state"], "partial");
        assert_eq!(
            edited["view"]["presentation"]["node_positions"]["greeting"]["x"],
            77
        );
        assert!(
            edited["view"]["topology"]["logical_nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == "greeting" && node["validity"] == "incomplete")
        );
        assert!(edited["view"].get("plan").is_none());
    }

    #[test]
    fn authored_cord_failures_have_distinct_truthful_states() {
        let cases = [
            (
                "receiving-source",
                "a: display/text\nb: display/text\na.text > b.text\n",
                "wrong-direction",
            ),
            (
                "unknown-node",
                "b: display/text\nmissing.value > b.text\n",
                "unresolved",
            ),
            (
                "unknown-port",
                "a: std/literal\nb: display/text\na.missing > b.text\n",
                "unresolved",
            ),
            (
                "incompatible",
                "a: std/literal\nb: io/stdout\na.value > b.bytes\n",
                "incompatible",
            ),
            (
                "bounds",
                "a: std/literal\nb: display/text\n\
                 a.value > b.text { capacity = 1 max_value_bytes = 8 \
                 max_queued_bytes = 8 low_watermark = 0 high_watermark = 2 pressure = block }\n",
                "invalid-bounds",
            ),
        ];
        for (id, body, expected) in cases {
            let source = format!("panel 0\n{body}");
            let opened: Value = serde_json::from_str(&patchbay_open_session(
                format!("test/cord-state-{id}"),
                source.clone(),
            ))
            .unwrap();
            assert_eq!(opened["ok"], true, "{id}: {opened}");
            assert_eq!(
                opened["view"]["topology"]["cords"][0]["validity"], expected,
                "{id}: {opened}"
            );
            if expected == "invalid-bounds" {
                assert!(
                    opened["view"]["diagnostics"][0]["targets"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|target| {
                            target["kind"] == "config"
                                && target["id"]
                                    .as_str()
                                    .is_some_and(|value| value.ends_with("/high_watermark"))
                        }),
                    "{id}: {opened}"
                );
                let range = &opened["view"]["diagnostics"][0]["primary_range"];
                let start = range["start_byte"].as_u64().unwrap() as usize;
                let end = range["end_byte"].as_u64().unwrap() as usize;
                assert!(source[start..end].starts_with("high_watermark"));
            }
            assert!(opened["view"].get("plan").is_none(), "{id}: {opened}");
        }
    }

    #[test]
    fn trivia_edit_rebases_diagnostic_ranges_to_the_new_revision() {
        let source = "panel 0\na: std/literal\nb: std/literal\n\
                      a.value > b.value\n";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/diagnostic-revision".to_owned(),
            source.to_owned(),
        ))
        .unwrap();
        let opened_range = &opened["view"]["diagnostics"][0]["primary_range"];
        let opened_start = opened_range["start_byte"].as_u64().unwrap() as usize;
        let opened_end = opened_range["end_byte"].as_u64().unwrap() as usize;
        assert_eq!(&source[opened_start..opened_end], "b.value");
        let replacement = serde_json::json!({
            "protocol_version": 0,
            "document_id": "test/diagnostic-revision",
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{"ReplaceSource": {
                "source": format!("# formatting only\n{source}")
            }}]
        });
        let edited: Value = serde_json::from_str(&patchbay_apply_transaction(
            "test/diagnostic-revision".to_owned(),
            replacement.to_string(),
        ))
        .unwrap();
        assert_eq!(edited["ok"], true, "{edited}");
        assert_eq!(
            edited["view"]["diagnostics"][0]["primary_range"]["source_revision"],
            1
        );
        assert!(
            edited["view"]["diagnostics"][0]["primary_range"]["start_byte"]
                .as_u64()
                .unwrap()
                > opened["view"]["diagnostics"][0]["primary_range"]["start_byte"]
                    .as_u64()
                    .unwrap()
        );
    }

    #[test]
    fn browser_evidence_commit_is_atomic_and_idempotent() {
        use std::{cell::RefCell, rc::Rc};

        use conduit_core::ArtifactDigest;
        use conduit_runtime::{
            ExactEvidenceCommitRequest, ExactEvidenceProvider, ExactEvidenceProviderBinding,
            ExactEvidenceRecord, ExactRunIdentity, exact_evidence_batch_digest,
        };
        use sha2::{Digest, Sha256};

        let artifact_digest =
            ArtifactDigest::from_bytes(Sha256::digest(include_bytes!("lib.rs")).into());
        let binding = ExactEvidenceProviderBinding {
            implementation_id: super::BROWSER_EVIDENCE_IMPLEMENTATION.to_owned(),
            implementation_identity: super::browser_evidence_hash(
                b"implementation",
                &[
                    super::BROWSER_EVIDENCE_IMPLEMENTATION.as_bytes(),
                    artifact_digest.as_bytes(),
                ],
            ),
            artifact_id: super::BROWSER_EVIDENCE_ARTIFACT.to_owned(),
            artifact_digest,
            host_observation_id: super::BROWSER_EVIDENCE_HOST_OBSERVATION.to_owned(),
            store_resource_kind: "evidence-store".to_owned(),
            store_resource_id: super::BROWSER_EVIDENCE_STORE.to_owned(),
            store_generation: 1,
            grant_hash: super::browser_evidence_hash(
                b"grant",
                &[
                    super::BROWSER_EVIDENCE_STORE.as_bytes(),
                    b"commit-exact-evidence",
                    b"clock/conduct-host",
                ],
            ),
            time_basis: "clock/conduct-host".to_owned(),
        };
        let run = ExactRunIdentity {
            plan_identity: SemanticHash::from_bytes([71; 32]),
            source_semantic_hash: SemanticHash::from_bytes([72; 32]),
            plan_epoch: 9,
            run_id: "fixture/browser-evidence-run".to_owned(),
        };
        let authority = Rc::new(RefCell::new(Some(super::browser_evidence_authority(
            &binding,
            &run.run_id,
            run.plan_epoch,
        ))));
        let store = Rc::new(RefCell::new(
            super::BrowserEvidenceStore::new(16 * 1024).unwrap(),
        ));
        let mut provider = super::BrowserEvidenceProvider {
            binding: binding.clone(),
            store: Rc::clone(&store),
            authority: Rc::clone(&authority),
        };
        let record = |sequence| ExactEvidenceRecord {
            schema: "conduit.exact-execution-evidence",
            schema_version: 0,
            plan_identity: run.plan_identity.to_string(),
            plan_epoch: run.plan_epoch,
            run_id: run.run_id.clone(),
            sequence,
            tick: sequence,
            subject_kind: "run",
            subject_id: run.run_id.clone(),
            node_id: None,
            semantic_contract_id: None,
            semantic_contract_descriptor_hash: None,
            cord_id: None,
            from_port: None,
            to_port: None,
            implementation_id: None,
            implementation_identity: None,
            artifact_id: None,
            host_id: None,
            host_observation_id: None,
            pressure: None,
            event_kind: "fixture",
            event_detail: None,
            terminal_cause: None,
            occupancy_items: 0,
            occupancy_bytes: 0,
            scheduling_latency_ticks: 0,
            processing_latency_ticks: 0,
        };
        let valid_records = vec![record(0), record(1)];
        let mut invalid_records = valid_records.clone();
        invalid_records[1].run_id = "fixture/wrong-run".to_owned();
        let request = ExactEvidenceCommitRequest {
            plan_identity: run.plan_identity,
            plan_epoch: run.plan_epoch,
            run_id: run.run_id.clone(),
            provider: binding,
            authority: provider.observe_use_authority(&run).unwrap(),
            start_cursor: 0,
            end_cursor: 2,
            batch_digest: exact_evidence_batch_digest(0, 2, &valid_records).unwrap(),
        };

        assert!(
            provider
                .commit_exact_evidence(&request, &invalid_records)
                .is_err()
        );
        assert!(store.borrow().records.is_empty());
        assert!(store.borrow().committed.is_empty());
        assert_eq!(store.borrow().next_cursor, 0);

        let first = provider
            .commit_exact_evidence(&request, &valid_records)
            .unwrap();
        let retry = provider
            .commit_exact_evidence(&request, &valid_records)
            .unwrap();
        assert_eq!(retry, first);
        assert_eq!(store.borrow().records.len(), 2);
        assert_eq!(store.borrow().committed.len(), 1);
        assert_eq!(store.borrow().next_cursor, 2);
    }
}
