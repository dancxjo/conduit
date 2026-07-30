//! Safe browser bindings to the production `.panel` parser.

use std::{cell::RefCell, collections::BTreeMap};

use wasm_bindgen::prelude::*;

use conduit_compile::{CompileInput, InstalledProfile, compile_source};
use conduit_core::{
    ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, TerminalClass,
};
use conduit_runtime::{ExactExecutionReport, ExactRunContext, RuntimeError, SchedulerReservation};

const MAXIMUM_PATCHBAY_SESSIONS: usize = 8;
const MAXIMUM_PATCHBAY_SESSION_ID_BYTES: usize = 256;
const MAXIMUM_PATCHBAY_REQUEST_BYTES: usize = 1024 * 1024;

thread_local! {
    static PATCHBAY_SESSIONS: RefCell<BTreeMap<String, conduit_patchbay::Workspace>> =
        const { RefCell::new(BTreeMap::new()) };
}

struct ExactBrowserResult {
    report: ExactExecutionReport,
    output: Vec<u8>,
    error: Vec<u8>,
    patchbay: serde_json::Value,
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
        let registry = conduit_runtime::Registry::hosted_primitives();
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
    conduit_runtime::Registry::hosted_primitives()
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
    let installed = InstalledProfile::observe(source).ok()?;
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
    let panel = conduit_panel::parse(&workspace.source().source).map_err(|error| {
        conduit_patchbay::ProtocolError {
            code: "CND-PBY-004",
            message: "Patchbay source projection failed parser validation".to_owned(),
            diagnostics: vec![error.to_string()],
            disposition: conduit_patchbay::EditDisposition::Rejected,
        }
    })?;
    let registry = conduit_runtime::Registry::hosted_primitives();
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| conduit_patchbay::ProtocolError {
            code: "CND-PBY-010",
            message: "Patchbay source projection failed resolver validation".to_owned(),
            diagnostics: vec![format!("{}: {}", error.code, error.message)],
            disposition: conduit_patchbay::EditDisposition::Rejected,
        })?;
    let resolved_view = resolved.view();
    let plan = exact_plan.or_else(|| exact_plan_snapshot(&workspace.source().source));
    let semantic = workspace.semantic_with_lookup(|id| availability_projection(&registry, id));
    let bounds = conduit_patchbay::PatchbayProjectionBounds::default();
    let source_text = workspace.source().source.as_str();
    let source_revision = workspace.source().revision;
    let mut expanded_nodes = resolved_view
        .nodes
        .iter()
        .map(|node| {
            project_resolved_node(
                node,
                &semantic,
                plan.as_ref(),
                run.as_ref(),
                BTreeMap::new(),
            )
        })
        .collect::<Vec<_>>();
    let expanded_by_id = expanded_nodes
        .iter()
        .map(|node| (node.id.clone(), node.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut logical_nodes = panel
        .nodes
        .iter()
        .map(|source_node| {
            let config = source_node
                .config
                .iter()
                .map(|entry| (entry.key.clone(), project_config_value(&entry.value)))
                .collect::<BTreeMap<_, _>>();
            if let Some(node) = expanded_by_id.get(&source_node.id) {
                let mut node = node.clone();
                node.contract_id.clone_from(&source_node.kind);
                node.config = config;
                node.source_range = declaration_source_range(
                    source_text,
                    source_node.source_span,
                    "node",
                    source_revision,
                    "authored",
                );
                node
            } else {
                let composite = resolved_view
                    .composites
                    .iter()
                    .find(|composite| composite.path == source_node.id);
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();
                if let Some(composite) = composite {
                    for export in &composite.exports {
                        let target = expanded_by_id.get(&export.target_node);
                        let target_port = target.and_then(|node| {
                            if export.direction == "input" {
                                node.inputs
                                    .iter()
                                    .find(|port| port.id == export.target_port)
                            } else {
                                node.outputs
                                    .iter()
                                    .find(|port| port.id == export.target_port)
                            }
                        });
                        let projected = conduit_patchbay::PatchbayPortProjection {
                            id: export.id.clone(),
                            semantic_path: format!(
                                "root/{}/port/{}/{}",
                                source_node.id,
                                if export.direction == "input" {
                                    "receiving"
                                } else {
                                    "outgoing"
                                },
                                export.id
                            ),
                            direction: export.direction.to_owned(),
                            display_label: if export.direction == "input" {
                                format!("> {}", export.id)
                            } else {
                                format!("{} >", export.id)
                            },
                            accessible_label: format!(
                                "{}, {} port",
                                export.id,
                                if export.direction == "input" {
                                    "receiving"
                                } else {
                                    "outgoing"
                                }
                            ),
                            type_id: target_port
                                .map_or("unknown", |port| port.type_id.as_str())
                                .to_owned(),
                            delivery: target_port
                                .map_or("unknown", |port| port.delivery.as_str())
                                .to_owned(),
                            connections: target_port
                                .map_or("unknown", |port| port.connections.as_str())
                                .to_owned(),
                            connected: panel.cords.iter().any(|cord| {
                                if export.direction == "input" {
                                    cord.to.node == source_node.id && cord.to.port == export.id
                                } else {
                                    cord.from.node == source_node.id && cord.from.port == export.id
                                }
                            }),
                        };
                        if export.direction == "input" {
                            inputs.push(projected);
                        } else {
                            outputs.push(projected);
                        }
                    }
                }
                conduit_patchbay::PatchbayNodeProjection {
                    id: source_node.id.clone(),
                    semantic_id: format!("root/{}", source_node.id),
                    contract_id: source_node.kind.clone(),
                    source_range: declaration_source_range(
                        source_text,
                        source_node.source_span,
                        "node",
                        source_revision,
                        "authored",
                    ),
                    inputs,
                    outputs,
                    config,
                    availability: availability_projection(&registry, &source_node.kind),
                    // Exact plans produced from InstalledProfile carry a
                    // deterministic compile input, not an independently
                    // observed host-placement fact. Keep the plan artifact
                    // visible without promoting its synthetic host report to
                    // authoritative Patchbay topology.
                    placement: None,
                    activity: None,
                }
            }
        })
        .collect::<Vec<_>>();
    for expanded in &mut expanded_nodes {
        if let Some(logical) = logical_nodes
            .iter()
            .find(|logical| logical.id == expanded.id)
        {
            expanded.config.clone_from(&logical.config);
        }
    }
    let mut cords = resolved_view
        .cords
        .iter()
        .map(|cord| {
            let producer_type = expanded_by_id
                .get(&cord.from_node)
                .and_then(|node| node.outputs.iter().find(|port| port.id == cord.from_port))
                .map(|port| port.type_id.clone())
                .unwrap_or_else(|| "unknown".to_owned());
            let consumer_type = expanded_by_id
                .get(&cord.to_node)
                .and_then(|node| node.inputs.iter().find(|port| port.id == cord.to_port))
                .map(|port| port.type_id.clone())
                .unwrap_or_else(|| "unknown".to_owned());
            let source_cord = panel
                .cords
                .iter()
                .find(|source_cord| source_cord.id == cord.id);
            let source_range = source_cord.and_then(|source_cord| {
                declaration_source_range(
                    source_text,
                    source_cord.source_span,
                    "cord",
                    source_revision,
                    "authored",
                )
            });
            let endpoint_ranges = source_cord
                .and_then(|source_cord| cord_endpoint_member_offsets(source_text, source_cord));
            let from_port_range = endpoint_ranges.and_then(|(from, _)| {
                source_range_from_offsets(source_text, from, source_revision, "authored-endpoint")
            });
            let to_port_range = endpoint_ranges.and_then(|(_, to)| {
                source_range_from_offsets(source_text, to, source_revision, "authored-endpoint")
            });
            conduit_patchbay::PatchbayCordProjection {
                id: cord.id.clone(),
                from_node: cord.from_node.clone(),
                from_port: cord.from_port.clone(),
                from_port_path: source_cord.map_or_else(
                    || format!("root/{}/port/outgoing/{}", cord.from_node, cord.from_port),
                    |source| {
                        format!(
                            "root/{}/port/outgoing/{}",
                            source.from.node, source.from.port
                        )
                    },
                ),
                from_port_range,
                to_node: cord.to_node.clone(),
                to_port: cord.to_port.clone(),
                to_port_path: source_cord.map_or_else(
                    || format!("root/{}/port/receiving/{}", cord.to_node, cord.to_port),
                    |source| format!("root/{}/port/receiving/{}", source.to.node, source.to.port),
                ),
                to_port_range,
                value_type: producer_type.clone(),
                compatibility: conduit_patchbay::CompatibilityProof {
                    compatible: true,
                    code: "CND-TYP-EXACT".to_owned(),
                    producer_type: Some(producer_type),
                    consumer_type: Some(consumer_type),
                    candidate_plan_identity: plan.as_ref().map(|plan| plan.identity.clone()),
                    plan_disposition: "observed-active-plan".to_owned(),
                },
                capacity_items: cord.capacity_items,
                max_value_bytes: cord.max_value_bytes,
                max_queued_bytes: cord.max_queued_bytes,
                low_watermark_items: cord.low_watermark_items,
                high_watermark_items: cord.high_watermark_items,
                pressure: cord.pressure.clone(),
                source_range,
                high_water_items: None,
            }
        })
        .collect::<Vec<_>>();
    for node in &mut expanded_nodes {
        for port in &mut node.inputs {
            port.connected = cords
                .iter()
                .any(|cord| cord.to_node == node.id && cord.to_port == port.id);
        }
        for port in &mut node.outputs {
            port.connected = cords
                .iter()
                .any(|cord| cord.from_node == node.id && cord.from_port == port.id);
        }
    }
    for node in &mut logical_nodes {
        for port in &mut node.inputs {
            port.connected = panel
                .cords
                .iter()
                .any(|cord| cord.to.node == node.id && cord.to.port == port.id);
        }
        for port in &mut node.outputs {
            port.connected = panel
                .cords
                .iter()
                .any(|cord| cord.from.node == node.id && cord.from.port == port.id);
        }
    }
    let mut composites = resolved_view
        .composites
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
        .collect::<Vec<_>>();
    let mut truncated = logical_nodes.len() > bounds.maximum_nodes
        || expanded_nodes.len() > bounds.maximum_nodes
        || cords.len() > bounds.maximum_cords
        || composites.len() > bounds.maximum_composites
        || evidence.len() > bounds.maximum_evidence_events;
    logical_nodes.truncate(bounds.maximum_nodes);
    expanded_nodes.truncate(bounds.maximum_nodes);
    cords.truncate(bounds.maximum_cords);
    composites.truncate(bounds.maximum_composites);
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
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_V1,
        source: workspace.source().clone(),
        semantic,
        presentation: workspace.presentation().clone(),
        plan,
        run,
        high_water,
        evidence: evidence
            .iter()
            .take(bounds.maximum_evidence_events)
            .cloned()
            .collect(),
        topology: conduit_patchbay::PatchbayTopologyProjection {
            logical_nodes,
            expanded_nodes,
            cords,
            composites,
        },
        bounds,
        truncated,
    })
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
        }
    };
    conduit_patchbay::PatchbayNodeProjection {
        id: node.id.clone(),
        semantic_id: format!("root/{}", node.id),
        contract_id: node.contract_id.clone(),
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
        availability: semantic
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
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_V1,
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
        protocol_version: conduit_patchbay::PATCHBAY_PROTOCOL_V1,
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
        "schema": "conduit.panel-language/v1",
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
            "schema": "conduit.panel-source-metadata/v1",
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
        "schema": "conduit.panel-source-metadata/v1",
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
    match conduit_runtime::Registry::hosted_primitives().resolve(&panel) {
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
    let mut io = conduit_runtime::RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
    };
    match resolved.run_batch(&mut io) {
        Ok(summary) => format!(
            "{{\"ok\":true,\"completed_nodes\":{},\"cords_conducted\":{},\"stdout\":{:?},\"stderr\":{:?}}}",
            summary.nodes_completed,
            summary.cords_conducted,
            String::from_utf8_lossy(&output),
            String::from_utf8_lossy(&error)
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
    let installed = InstalledProfile::observe(source)?;
    let explicit_input = compile_input_json
        .map(|json| {
            serde_json::from_str::<CompileInput>(json)
                .map_err(|_| RuntimeError::new("CND-CMP-002", "invalid compile-input/v2 JSON"))
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
    let registry = conduit_runtime::Registry::hosted_primitives();
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    let mut input_stream = std::io::empty();
    let mut output = Vec::new();
    let mut error = Vec::new();
    let report = {
        let mut io = conduit_runtime::RunIo {
            input: &mut input_stream,
            output: &mut output,
            error: &mut error,
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
                max_events: if plan.nodes.len() > 5 { 256 } else { 128 },
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

    const SOURCE: &str = "panel 1\nnode greeting : std/literal { value = \"hello\\n\" }\nnode output : io/stdout\ncord greeting.out -> output.in\n";

    #[test]
    fn parser_metadata_and_patchbay_ranges_are_authoritative() {
        let metadata: Value =
            serde_json::from_str(&panel_language_metadata()).expect("language metadata JSON");
        assert_eq!(metadata["schema"], "conduit.panel-language/v1");
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
            "cord greeting.out -> output.in"
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
            ("from_port_range", "out", "root/greeting/port/outgoing/out"),
            ("to_port_range", "in", "root/output/port/receiving/in"),
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
        let source = "panel 3\n\
interface fixture/duplex {\n\
  > in : fixture/text\n\
  in > : fixture/text\n\
  > audio : fixture/audio\n\
  committed > : fixture/text\n\
}\n\
composite fixture/box {\n\
  node worker : fixture/sink\n\
  export > audio = worker.in\n\
}\n\
node output : fixture/source\n\
node sink : fixture/sink\n\
cord output.in -> sink.in\n\
# > comment.in and \"string.out >\" are not ports\n";
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
            "in",
            "receiving",
            "interface/fixture/duplex/port/receiving/in"
        )));
        assert!(names.contains(&(
            "in",
            "outgoing",
            "interface/fixture/duplex/port/outgoing/in"
        )));
        assert!(names.iter().any(|entry| {
            entry.0 == "in" && entry.1 == "outgoing" && entry.2.ends_with("/from/output/in")
        }));
        assert!(names.iter().any(|entry| {
            entry.0 == "in" && entry.1 == "receiving" && entry.2.ends_with("/to/sink/in")
        }));
        assert!(names.iter().any(|entry| {
            entry.0 == "in"
                && entry.1 == "receiving"
                && entry.2.ends_with("/export/audio/target/worker/in")
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
            "panel 3\ncord source.value ->\n".to_owned(),
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
            "panel 1\n\
             composite example/upper {\n\
               node worker : text/uppercase\n\
               export input in = worker.in\n\
               export output out = worker.out\n\
             }\n\
             node source : std/literal { value = \"hello\" }\n\
             node transform : example/upper\n\
             node sink : io/stdout\n\
             cord source.out -> transform.in\n\
             cord transform.out -> sink.in\n"
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
        assert_eq!(opened["view"]["protocol_version"], 1);
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["type_id"],
            "std/text"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["display_label"],
            "out >"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["accessible_label"],
            "out, outgoing port"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][0]["outputs"][0]["semantic_path"],
            "root/greeting/port/outgoing/out"
        );
        assert_eq!(
            opened["view"]["topology"]["logical_nodes"][1]["inputs"][0]["display_label"],
            "> in"
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
            "protocol_version": 1,
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
            "protocol_version": 1,
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
        let composite = "panel 1\n\
composite example/box {\n\
  node worker : text/uppercase\n\
  export input in = worker.in\n\
  export output out = worker.out\n\
}\n\
node source : std/literal { value = \"hello\" }\n\
node box : example/box\n\
node sink : io/stdout\n\
cord source.out -> box.in\n\
cord box.out -> sink.in\n";
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
            "protocol_version": 1,
            "document_id": "test/composite",
            "expected_source_revision": 0,
            "expected_presentation_revision": 0,
            "operations": [{
                "Connect": {
                    "from_node": "source",
                    "from_port": "out",
                    "to_node": "box.worker",
                    "to_port": "in",
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
        let mut source = String::from("panel 1\n");
        for index in 0..513 {
            source.push_str(&format!(
                "node literal_{index} : std/literal {{ value = \"{index}\" }}\n\
                 node output_{index} : io/stdout\n\
                 cord literal_{index}.out -> output_{index}.in\n"
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
}
