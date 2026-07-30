//! Safe browser bindings to the production `.panel` parser.

use wasm_bindgen::prelude::*;

use conduit_compile::{CompileInput, InstalledProfile, compile_source};
use conduit_core::{
    ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, TerminalClass,
};
use conduit_runtime::{ExactExecutionReport, ExactRunContext, RuntimeError, SchedulerReservation};

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
    format!(
        "{{\"ok\":false,\"diagnostic\":{:?},\"code\":{:?}}}",
        error.to_string(),
        error.code
    )
}

fn patchbay_result(result: conduit_patchbay::EditResult) -> String {
    format!(
        "{{\"ok\":true,\"source_revision\":{},\"presentation_revision\":{},\"semantic_hash\":{:?},\"presentation_identity\":{:?},\"positions\":{}}}",
        result.source.revision,
        result.presentation.revision,
        result.semantic.source_semantic_hash,
        result.presentation.identity,
        serde_json::to_string(&result.presentation.node_positions)
            .expect("Patchbay positions are serializable")
    )
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
    let semantic = workspace.semantic_with_lookup(|contract_id| {
        plan_snapshot
            .bindings
            .iter()
            .find(|binding| binding.contract_id == contract_id)
            .map_or_else(
                || conduit_patchbay::NodeAvailabilityProjection {
                    contract_id: contract_id.to_owned(),
                    availability_state: "unavailable".to_owned(),
                    reason_code: "CND-AVL-006".to_owned(),
                    implementation_id: None,
                    host_id: None,
                    rejection_reasons: vec!["not bound in the exact plan".to_owned()],
                },
                |binding| conduit_patchbay::NodeAvailabilityProjection {
                    contract_id: contract_id.to_owned(),
                    availability_state: binding.availability_state.clone(),
                    reason_code: binding.reason_code.clone(),
                    implementation_id: Some(binding.implementation_id.clone()),
                    host_id: Some(binding.host_id.clone()),
                    rejection_reasons: Vec::new(),
                },
            )
    });
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
                max_events: 64,
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
    let patchbay = serde_json::json!({
        "source": workspace.source(),
        "semantic": semantic,
        "presentation": workspace.presentation(),
        "plan": plan_snapshot,
        "run": run,
        "evidence": &report.evidence,
    });
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

    use super::{explain_panel, patchbay_move_node, patchbay_replace_source};

    const SOURCE: &str = "panel 1\nnode greeting : conduit.std/literal { value = \"hello\\n\" }\nnode output : conduit.std/stdout\ncord greeting.out -> output.in\n";

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
               node worker : conduit.std/uppercase\n\
               export input in = worker.in\n\
               export output out = worker.out\n\
             }\n\
             node source : conduit.std/literal { value = \"hello\" }\n\
             node transform : example/upper\n\
             node sink : conduit.std/stdout\n\
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
            value.contains("transform.worker : conduit.std/uppercase")
                || value.contains("transform.worker : conduit.std/uppercase")
        }));
    }
}
