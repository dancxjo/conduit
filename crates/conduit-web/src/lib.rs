//! Safe browser bindings to the production `.panel` parser.

use std::{cell::RefCell, collections::BTreeMap};

use wasm_bindgen::prelude::*;

use conduit_compile::{CompileInput, InstalledProfile, compile_source};
use conduit_core::{
    ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, SemanticHash, TerminalClass,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, ExactExecutionReport, ExactRunContext, Handler, Registry,
    ResolutionError, RunIo, RuntimeError, SchedulerReservation, Value, file_read_contract,
    file_watch_contract, file_write_contract,
};
use conduit_std::{
    FileHandle, FileSlot, FlushClaim, MemoryFilesystem, PartialWritePolicy, ReadConsistency,
    ReadRequest, WatchCoalescing, WatchOverflow, WatchRequest, WriteMode, WriteRequest,
};

const MAXIMUM_PATCHBAY_SESSIONS: usize = 8;
const MAXIMUM_PATCHBAY_SESSION_ID_BYTES: usize = 256;
const MAXIMUM_PATCHBAY_REQUEST_BYTES: usize = 1024 * 1024;
const BROWSER_READ_RESOURCE: &str = "conduit.resource/filesystem-example-read";
const BROWSER_WRITE_RESOURCE: &str = "conduit.resource/filesystem-example-write";
const BROWSER_WATCH_RESOURCE: &str = "conduit.resource/filesystem-example-watch";
const BROWSER_FILE_BYTES: &[u8] = b"bounded filesystem fixture\n";
const MONOTONIC_CLOCK_HASH: &[u8; 32] = &[
    0x6b, 0x9c, 0x68, 0x72, 0x26, 0xd4, 0xa1, 0x96, 0x5e, 0x78, 0x0b, 0x63, 0xb4, 0xbd, 0xc0, 0x92,
    0x2d, 0xe2, 0xa6, 0x86, 0xc3, 0xc1, 0x36, 0x5f, 0x4f, 0x68, 0xf7, 0x21, 0x9f, 0x30, 0xcc, 0x48,
];

thread_local! {
    static PATCHBAY_SESSIONS: RefCell<BTreeMap<String, conduit_patchbay::Workspace>> =
        const { RefCell::new(BTreeMap::new()) };
}

struct ExactBrowserResult {
    report: ExactExecutionReport,
    output: Vec<u8>,
    error: Vec<u8>,
    display: Vec<u8>,
    patchbay: serde_json::Value,
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
        let view = match authoritative_patchbay_view(&workspace, None, None, None, &[]) {
            Ok(view) => view,
            Err(error) => return patchbay_error(error),
        };
        sessions.insert(document_id.clone(), workspace);
        serde_json::json!({
            "ok": true,
            "session_id": document_id,
            "view": view,
        })
        .to_string()
    })
}

/// Returns the current authoritative Rust projection for a Patchbay session.
#[wasm_bindgen]
pub fn patchbay_session_view(session_id: String) -> String {
    PATCHBAY_SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let Some(workspace) = sessions.get(&session_id) else {
            return serde_json::json!({
                "ok": false,
                "code": "CND-PBY-011",
                "diagnostic": "unknown Patchbay session",
                "diagnostics": [],
                "disposition": "rejected",
            })
            .to_string();
        };
        match authoritative_patchbay_view(workspace, None, None, None, &[]) {
            Ok(view) => serde_json::json!({"ok": true, "view": view}).to_string(),
            Err(error) => patchbay_rejection(error, workspace),
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
        let Some(workspace) = sessions.get_mut(&session_id) else {
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
        let result = workspace.apply_validated(
            request,
            |contract_id| availability_projection(&registry, contract_id),
            validate_patchbay_candidate,
        );
        match result {
            Ok(result) => match authoritative_patchbay_view(workspace, None, None, None, &[]) {
                Ok(view) => serde_json::json!({
                    "ok": true,
                    "result": result,
                    "view": view,
                    "history_retained": workspace.history().len(),
                })
                .to_string(),
                Err(error) => patchbay_rejection(error, workspace),
            },
            Err(error) => patchbay_rejection(error, workspace),
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
    let installed = InstalledProfile::observe_registry(source, &registry).ok()?;
    let document = compile_source(source, &installed.input).ok()?;
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).ok()?;
    Some(conduit_patchbay::PlanSnapshot::from_exact_plan(&plan))
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
            let node_range = declaration_source_range(
                source_text,
                source_node.source_span,
                "node",
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
                    let mut projection = project_config_value(&entry.value);
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
                source_range: node_range,
                inputs,
                outputs,
                config,
                availability: contract
                    .as_ref()
                    .map(|_| availability_projection(&registry, &source_node.kind)),
                validity: validity.to_owned(),
                diagnostic_ids,
                placement: None,
                activity: None,
            });
        }
        for source_cord in &panel.cords {
            let source_range = declaration_source_range(
                source_text,
                source_cord.source_span,
                "cord",
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
                expanded_from_node: None,
                expanded_from_port: None,
                expanded_to_node: None,
                expanded_to_port: None,
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
                source_range: range,
                inputs,
                outputs,
                config: BTreeMap::new(),
                availability: recovered_node.kind.as_deref().and_then(|kind| {
                    contract
                        .as_ref()
                        .map(|_| availability_projection(&registry, kind))
                }),
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
                expanded_from_node: None,
                expanded_from_port: None,
                expanded_to_node: None,
                expanded_to_port: None,
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
    let resolved = document
        .ast
        .as_ref()
        .and_then(|panel| registry.resolve(panel).ok());
    let resolved_view = resolved.as_ref().map(conduit_runtime::ResolvedPanel::view);
    if let Some(view) = resolved_view.as_ref() {
        for cord in &mut cords {
            if let Some(expanded) = view.cords.iter().find(|candidate| candidate.id == cord.id) {
                cord.expanded_from_node = Some(expanded.from_node.clone());
                cord.expanded_from_port = Some(expanded.from_port.clone());
                cord.expanded_to_node = Some(expanded.to_node.clone());
                cord.expanded_to_port = Some(expanded.to_port.clone());
            }
        }
    }
    let plan = resolved_view
        .as_ref()
        .and_then(|_| exact_plan.or_else(|| exact_plan_snapshot(source_text)));
    let matching_run = run.filter(|run| {
        plan.as_ref().is_some_and(|plan| {
            run.plan_identity == plan.identity
                && workspace.source().semantic_hash.as_deref()
                    == Some(run.source_semantic_hash.as_str())
        })
    });
    let mut expanded_nodes = resolved_view.as_ref().map_or_else(Vec::new, |view| {
        view.nodes
            .iter()
            .map(|node| {
                project_resolved_node(
                    node,
                    &semantic,
                    plan.as_ref(),
                    matching_run.as_ref(),
                    BTreeMap::new(),
                )
            })
            .collect()
    });
    let mut composites = resolved_view.as_ref().map_or_else(Vec::new, |view| {
        view.composites
            .iter()
            .map(|composite| conduit_patchbay::PatchbayCompositeProjection {
                id: composite.path.clone(),
                definition: composite.definition.clone(),
                members: composite
                    .children
                    .iter()
                    .map(|child| child.path.clone())
                    .collect(),
                exports: composite
                    .exports
                    .iter()
                    .map(|export| conduit_patchbay::PatchbayExportProjection {
                        direction: export.direction.to_owned(),
                        id: export.id.clone(),
                        target_node: export.target_node.clone(),
                        target_port: export.target_port.clone(),
                    })
                    .collect(),
            })
            .collect()
    });
    let mut truncated = logical_nodes.len() > bounds.maximum_nodes
        || expanded_nodes.len() > bounds.maximum_nodes
        || cords.len() > bounds.maximum_cords
        || composites.len() > bounds.maximum_composites
        || diagnostics.len() > bounds.maximum_diagnostics
        || evidence.len() > bounds.maximum_evidence_events;
    logical_nodes.truncate(bounds.maximum_nodes);
    expanded_nodes.truncate(bounds.maximum_nodes);
    cords.truncate(bounds.maximum_cords);
    composites.truncate(bounds.maximum_composites);
    diagnostics.truncate(bounds.maximum_diagnostics);
    for node in logical_nodes.iter_mut().chain(expanded_nodes.iter_mut()) {
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
    for composite in &mut composites {
        truncated |= composite.members.len() > bounds.maximum_nodes
            || composite.exports.len() > bounds.maximum_ports_per_node;
        composite.members.truncate(bounds.maximum_nodes);
        composite.exports.truncate(bounds.maximum_ports_per_node);
    }
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
            expanded_nodes,
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
    conduit_patchbay::PatchbayPortProjection {
        id: port.id.clone(),
        semantic_path: format!("root/{node_id}/port/{presentation_direction}/{}", port.id),
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

fn project_resolved_node(
    node: &conduit_runtime::ResolvedNodeView,
    semantic: &conduit_patchbay::SemanticSnapshot,
    _plan: Option<&conduit_patchbay::PlanSnapshot>,
    _run: Option<&conduit_patchbay::RunSnapshot>,
    config: BTreeMap<String, conduit_patchbay::PatchbayConfigProjection>,
) -> conduit_patchbay::PatchbayNodeProjection {
    let project_port = |port: &conduit_runtime::ResolvedPortView, direction: &str| {
        let presentation_direction = if direction == "input" {
            "receiving"
        } else {
            "outgoing"
        };
        conduit_patchbay::PatchbayPortProjection {
            id: port.id.clone(),
            semantic_path: format!(
                "root/{}/port/{}/{}",
                node.id, presentation_direction, port.id
            ),
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
            connected: false,
            source_range: None,
            validity: "valid".to_owned(),
            diagnostic_ids: Vec::new(),
        }
    };
    conduit_patchbay::PatchbayNodeProjection {
        id: node.id.clone(),
        semantic_id: format!("root/{}", node.id),
        contract_id: Some(node.contract_id.clone()),
        source_range: None,
        inputs: node
            .inputs
            .iter()
            .map(|port| project_port(port, "input"))
            .collect(),
        outputs: node
            .outputs
            .iter()
            .map(|port| project_port(port, "output"))
            .collect(),
        config,
        availability: Some(
            semantic
                .availabilities
                .iter()
                .find(|availability| availability.contract_id == node.contract_id)
                .cloned()
                .unwrap_or_else(|| conduit_patchbay::NodeAvailabilityProjection {
                    contract_id: node.contract_id.clone(),
                    availability_state: "unsupported".to_owned(),
                    reason_code: "CND-AVL-006".to_owned(),
                    implementation_id: None,
                    host_id: None,
                    rejection_reasons: vec!["no authoritative availability observation".to_owned()],
                }),
        ),
        validity: "valid".to_owned(),
        diagnostic_ids: Vec::new(),
        // Placement is an observed host fact, not something the presentation
        // layer may infer from an exact plan compiled against InstalledProfile.
        placement: None,
        activity: None,
    }
}

fn declaration_source_range(
    source: &str,
    span: conduit_panel::SourceSpan,
    declaration: &str,
    source_revision: u64,
    provenance: &str,
) -> Option<conduit_patchbay::SourceRangeProjection> {
    fn offset(source: &str, line: usize, column: usize) -> Option<usize> {
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

    let span_start = offset(source, span.line, span.column)?;
    let end_byte = offset(source, span.end_line, span.end_column)?;
    if span_start > end_byte || end_byte > source.len() {
        return None;
    }
    let line_start = source[..span_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = &source[line_start..span_start];
    let declaration_offset = prefix.find(declaration)?;
    let start_byte = line_start + declaration_offset;
    let keyword_end = start_byte + declaration.len();
    if source
        .as_bytes()
        .get(keyword_end)
        .is_some_and(|byte| *byte != b' ')
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

fn project_config_value(
    value: &conduit_panel::SourceValue,
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
                .map(|node| format!("{:?}", format!("{} : {}", node.id, node.kind)))
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
    let sigil = if direction == "receiving" {
        before.rfind('>').map(|relative| start + relative)
    } else {
        after
            .find('>')
            .map(|relative| id_start + id.len() + relative)
            .or_else(|| before.rfind('<').map(|relative| start + relative))
    };
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
    let complete = format!("{}.{}", endpoint.node, endpoint.port);
    let region = source.get(search_start..search_end)?;
    let relative = if reverse {
        region.rfind(&complete)
    } else {
        region.find(&complete)
    }?;
    let member_start = search_start + relative + complete.len() - endpoint.port.len();
    Some((member_start, member_start + endpoint.port.len()))
}

fn cord_endpoint_member_offsets(
    source: &str,
    cord: &conduit_panel::Cord,
) -> Option<((usize, usize), (usize, usize))> {
    let (start, end) = source_span_offsets(source, cord.source_span)?;
    let declaration = source.get(start..end)?;
    let body_start = declaration.find('{').unwrap_or(declaration.len());
    let endpoints = &declaration[..body_start];
    if let Some(relative) = endpoints.find("->") {
        let arrow = start + relative;
        let from = endpoint_member_offset(source, (start, arrow), &cord.from, true)?;
        let to = endpoint_member_offset(source, (arrow + 2, start + body_start), &cord.to, false)?;
        Some((from, to))
    } else {
        let relative = endpoints.find("<-")?;
        let arrow = start + relative;
        let to = endpoint_member_offset(source, (start, arrow), &cord.to, true)?;
        let from =
            endpoint_member_offset(source, (arrow + 2, start + body_start), &cord.from, false)?;
        Some((from, to))
    }
}

fn annotate_cords(
    source: &str,
    annotations: &mut Vec<serde_json::Value>,
    owner: &str,
    cords: &[conduit_panel::Cord],
) {
    for cord in cords {
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
    let installed = InstalledProfile::observe_registry(source, &registry)?;
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
    let plan_snapshot = conduit_patchbay::PlanSnapshot::from_exact_plan(&plan);
    let workspace = conduit_patchbay::Workspace::new("conduit/browser-source", source)
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))?;
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
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
    use serde_json::Value;

    use super::{
        explain_panel, panel_language_metadata, panel_source_metadata, patchbay_apply_transaction,
        patchbay_move_node, patchbay_open_session, patchbay_replace_source, patchbay_session_view,
    };

    const SOURCE: &str = "panel 0\nnode greeting : std/literal { value = \"hello\\n\" }\nnode output : display/text\ncord greeting.value -> output.text\n";

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
            "cord greeting.value -> output.text"
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
  > value : fixture/text\n\
  result > : fixture/text\n\
  > audio : fixture/audio\n\
  committed > : fixture/text\n\
}\n\
composite fixture/box {\n\
  node worker : fixture/sink\n\
  export > audio = worker.result\n\
}\n\
node output : fixture/source\n\
node sink : fixture/sink\n\
cord output.value -> sink.result\n\
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
        assert_eq!(
            annotations
                .iter()
                .filter(|entry| entry["kind"] == "port-sigil")
                .count(),
            5
        );
        assert!(!annotations.iter().any(|entry| {
            let start = entry["start_byte"].as_u64().unwrap() as usize;
            source[..start].ends_with("comment.")
        }));

        let malformed: Value = serde_json::from_str(&panel_source_metadata(
            "panel 0\ncord source.value ->\n".to_owned(),
        ))
        .unwrap();
        assert_eq!(malformed["semantic_available"], false);
        assert_eq!(malformed["annotations"], serde_json::json!([]));
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
             composite example/upper {\n\
               node worker : text/uppercase\n\
               export > text = worker.text\n\
               export uppercased > = worker.text\n\
             }\n\
             node source : std/literal { value = \"hello\" }\n\
             node transform : example/upper\n\
             node sink : display/text\n\
             cord source.value -> transform.text\n\
             cord transform.uppercased -> sink.text\n"
                .to_owned(),
        ))
        .expect("explanation JSON");
        assert_eq!(explained["ok"], true);
        assert!(
            explained["logical"]
                .as_str()
                .is_some_and(|value| value.contains("composite transform : example/upper"))
        );
        assert!(explained["expanded"].as_str().is_some_and(|value| {
            value.contains("transform.worker : text/uppercase")
                || value.contains("transform.worker : text/uppercase")
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
        for layer in ["logical_nodes", "expanded_nodes"] {
            let nodes = opened["view"]["topology"][layer]
                .as_array()
                .expect("projected nodes");
            assert!(!nodes.is_empty());
            assert!(
                nodes.iter().all(|node| node.get("placement").is_none()),
                "synthetic installed-profile host facts must not be projected as placement"
            );
        }
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
    fn candidate_connection_rejects_hidden_composite_members() {
        let composite = "panel 0\n\
composite example/box {\n\
  node worker : text/uppercase\n\
  export > text = worker.text\n\
  export uppercased > = worker.text\n\
}\n\
node source : std/literal { value = \"hello\" }\n\
node box : example/box\n\
node sink : display/text\n\
cord source.value -> box.text\n\
cord box.uppercased -> sink.text\n";
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
            opened["view"]["topology"]["expanded_nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == "box.worker")
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
                "node literal_{index} : std/literal {{ value = \"{index}\" }}\n\
                 node output_{index} : display/text\n\
                 cord literal_{index}.value -> output_{index}.text\n"
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
node first : std/literal { value = \"First.\\n\" }\n\
node second : std/literal { value = \"Second.\\n\" }\n\
cord first.value -> second.value {\n\
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
        let incomplete = "panel 0\nnode greeting : std/literal {\n value =\nnode preserved :";
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
                "node a : display/text\nnode b : display/text\ncord a.text -> b.text\n",
                "wrong-direction",
            ),
            (
                "unknown-node",
                "node b : display/text\ncord missing.value -> b.text\n",
                "unresolved",
            ),
            (
                "unknown-port",
                "node a : std/literal\nnode b : display/text\ncord a.missing -> b.text\n",
                "unresolved",
            ),
            (
                "incompatible",
                "node a : std/literal\nnode b : io/stdout\ncord a.value -> b.bytes\n",
                "incompatible",
            ),
            (
                "bounds",
                "node a : std/literal\nnode b : display/text\n\
                 cord a.value -> b.text { capacity = 1 max_value_bytes = 8 \
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
        let source = "panel 0\nnode a : std/literal\nnode b : std/literal\n\
                      cord a.value -> b.value\n";
        let opened: Value = serde_json::from_str(&patchbay_open_session(
            "test/diagnostic-revision".to_owned(),
            source.to_owned(),
        ))
        .unwrap();
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
}
