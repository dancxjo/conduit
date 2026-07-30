//! Safe browser bindings to the production `.panel` parser.

use wasm_bindgen::prelude::*;

use conduit_compile::{CompileInput, compile_source};
use conduit_core::{
    ExecutionPlan, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
};
use conduit_runtime::{
    ExactHostedBinding, ExactHostedBindings, ExactRunContext, HostedPrimitiveImplementation,
    RuntimeError, SchedulerReservation,
};

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
    match conduit_runtime::Registry::compatibility_demo().resolve(&panel) {
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
    match run_panel_exact_inner(&source, &compile_input_json) {
        Ok((summary, output, error)) => serde_json::json!({
            "ok": true,
            "completed_nodes": summary.nodes_completed,
            "cords_conducted": summary.cords_conducted,
            "stdout": String::from_utf8_lossy(&output),
            "stderr": String::from_utf8_lossy(&error),
            "profile": "exact-plan-deterministic-executor",
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
    compile_input_json: &str,
) -> Result<(conduit_runtime::ExecutionSummary, Vec<u8>, Vec<u8>), RuntimeError> {
    let panel = conduit_panel::parse(source)
        .map_err(|error| RuntimeError::new("CND-SRC-001", error.to_string()))?;
    let input: CompileInput = serde_json::from_str(compile_input_json)
        .map_err(|_| RuntimeError::new("CND-CMP-002", "invalid compile-input/v2 JSON"))?;
    let document = compile_source(source, &input)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let arena = bumpalo::Bump::new();
    let plan = document
        .as_plan(&arena)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let bindings = exact_hosted_bindings(&input, &plan)?;
    let registry = conduit_runtime::Registry::compatibility_demo();
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| RuntimeError::new(error.code, error.message))?;
    let mut input_stream = std::io::empty();
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut io = conduit_runtime::RunIo {
        input: &mut input_stream,
        output: &mut output,
        error: &mut error,
    };
    let summary = resolved.run_exact(
        &plan,
        &bindings,
        ExactRunContext {
            semantic_source_hash: plan.source_semantic_hash,
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
        },
        &mut io,
    )?;
    drop(io);
    Ok((summary, output, error))
}

fn exact_hosted_bindings(
    input: &CompileInput,
    plan: &ExecutionPlan<'_>,
) -> Result<ExactHostedBindings, RuntimeError> {
    let mut bindings = Vec::with_capacity(plan.nodes.len());
    for node in plan.nodes {
        let candidate = input
            .candidates
            .iter()
            .find(|candidate| {
                candidate.implementation.id == node.implementation.id.as_str()
                    && candidate.implementation.identity
                        == node.implementation.semantic_hash.to_string()
                    && candidate.host_report.id == node.host_observation.as_str()
            })
            .ok_or_else(|| RuntimeError::new("CND-RUN-007", "exact implementation is absent"))?;
        if candidate.implementation.entrypoint_adapter != "conduit/hosted-primitive-step"
            || candidate.implementation.entrypoint_abi != "conduit/hosted-primitive-v1"
            || candidate.implementation.runtime_protocol_version != 1
        {
            return Err(RuntimeError::new(
                "CND-RUN-007",
                "exact implementation uses an unavailable browser adapter",
            ));
        }
        let implementation = match candidate.implementation.entrypoint_name.as_str() {
            "literal" => HostedPrimitiveImplementation::Literal,
            "stdin" => HostedPrimitiveImplementation::Stdin,
            "uppercase" => HostedPrimitiveImplementation::Uppercase,
            "stdout" => HostedPrimitiveImplementation::Stdout,
            "stderr" => HostedPrimitiveImplementation::Stderr,
            "pass-through" => HostedPrimitiveImplementation::PassThrough,
            "tee" => HostedPrimitiveImplementation::Tee,
            "merge" => HostedPrimitiveImplementation::Merge,
            "fallback" => HostedPrimitiveImplementation::Fallback,
            _ => {
                return Err(RuntimeError::new(
                    "CND-RUN-007",
                    "exact implementation names an unavailable browser entrypoint",
                ));
            }
        };
        let artifact = plan
            .artifacts
            .iter()
            .find(|artifact| artifact.id == node.artifact)
            .ok_or_else(|| RuntimeError::new("CND-RUN-008", "exact artifact is absent"))?;
        bindings.push(ExactHostedBinding {
            implementation_id: node.implementation.id.to_string(),
            implementation_identity: node.implementation.semantic_hash,
            artifact_id: node.artifact.to_string(),
            artifact_digest: artifact.digest,
            implementation,
        });
    }
    ExactHostedBindings::new(bindings)
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
