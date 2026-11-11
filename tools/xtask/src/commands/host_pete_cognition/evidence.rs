//! Bounded exports of actual proof inputs, outputs and execution identities.
use super::execution::{Capture, Clock, ProviderCapture};
use conduit_ai::LocalModelOffer;
use conduit_body::BodyPlan;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{body_execution::BodyRunReport, ExternalForeInput};
use serde::Serialize;
use serde_json::json;
use std::{error::Error, fs, path::Path};
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
#[derive(Serialize)]
pub(super) struct Criterion {
    pub name: String,
    pub passed: bool,
}
#[derive(Serialize)]
pub(super) struct Receipt {
    pub schema: &'static str,
    pub proof_class: &'static str,
    pub input_class: &'static str,
    pub accepted: bool,
    pub body_id: String,
    pub wake_id: String,
    pub plan_id: String,
    pub active_play_id: Option<String>,
    pub model_content_identity: String,
    pub model_calls: usize,
    pub responses: usize,
    pub completed: Option<u64>,
    pub retained_records: usize,
    pub clock_waits: usize,
    pub clock_waits_after_feedback: usize,
    pub clock_ticks: usize,
    pub timed_out: bool,
    pub failure: Option<String>,
    pub criteria: Vec<Criterion>,
    pub causal_export: String,
    pub debugger_export: serde_json::Value,
    pub final_response_causal_export: serde_json::Value,
    pub artifacts: Vec<String>,
}
pub(super) fn write_preparation(
    output: &Path,
    plan: &BodyPlan,
    inputs: &[ExternalForeInput],
    model: &LocalModelOffer,
    body: &conduit_body::Body,
    wake: &conduit_body::Wake,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output)?;
    json_file(output, "body-plan.json", plan)?;
    json_file(output, "body.json", body)?;
    json_file(output, "wake.json", wake)?;
    json_file(output, "model-offer.json", model)?;
    for (index, input) in inputs.iter().enumerate() {
        fs::write(
            output.join(format!("input-frame-{}.canonical", index + 1)),
            &input.bytes,
        )?;
    }
    json_file(
        output,
        "preparation.json",
        &json!({
            "schema": "conduit.pete/live-cognition-preparation@1",
            "input_class": "two scripted typed frames; no physical sensing or actuation",
            "body_snapshot_scope": "historical state at Wake before Play; not a current liveness observation",
            "first_observation": super::plans::FIRST_OBSERVATION,
            "second_observation": super::plans::SECOND_OBSERVATION,
            "body_id": plan.body_id.as_str(), "wake_id": plan.wake_id.as_str(),
            "body_plan_id": plan.plan_id.as_str(), "input_frames": inputs.len(),
        }),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn write_run(
    output: &Path,
    plan: &BodyPlan,
    report: Option<&BodyRunReport>,
    model: &LocalModelOffer,
    capture: &Capture,
    provider: &ProviderCapture,
    clock: &Clock,
    clock_output: &[u8],
    failure: Option<String>,
    criteria: Vec<Criterion>,
) -> Result<Receipt, Box<dyn Error>> {
    let mut artifacts = vec![
        "body-plan.json".into(),
        "body.json".into(),
        "wake.json".into(),
        "model-offer.json".into(),
        "preparation.json".into(),
        "input-frame-1.canonical".into(),
        "input-frame-2.canonical".into(),
    ];
    for (prefix, values) in [
        ("situation", &capture.situations),
        ("response", &capture.responses),
        ("autobiography", &capture.states),
        ("loss", &capture.losses),
    ] {
        for (index, bytes) in values.iter().enumerate() {
            let extension = if prefix == "response" {
                "json"
            } else if prefix == "loss" {
                "terminal"
            } else {
                "canonical"
            };
            let name = format!("{prefix}-{}.{extension}", index + 1);
            fs::write(output.join(&name), bytes)?;
            artifacts.push(name);
            if prefix == "response" {
                let name = format!("response-{}.txt", index + 1);
                let text = match serde_json::from_slice::<conduit_ai::ModelDerivedResult>(bytes) {
                    Ok(result) => String::from_utf8(result.payload)
                        .unwrap_or_else(|_| "[non-UTF8 actual model payload]".into()),
                    Err(error) => format!("[actual result envelope did not decode: {error}]"),
                };
                fs::write(output.join(&name), text)?;
                artifacts.push(name);
            }
        }
    }
    for (index, request) in provider.requests[..provider.calls].iter().enumerate() {
        let name = format!("model-request-{}.json", index + 1);
        fs::write(output.join(&name), request)?;
        artifacts.push(name);
    }
    json_file(
        output,
        "provider-calls.json",
        &json!({"calls":provider.calls,
        "terminal_outcomes": provider.terminals.iter().map(|value| value.map(|value| format!("{value:?}"))).collect::<Vec<_>>() }),
    )?;
    artifacts.push("provider-calls.json".into());
    fs::write(output.join("clock-output.txt"), clock_output)?;
    artifacts.push("clock-output.txt".into());
    if let Some(report) = report {
        json_file(
            output,
            "body-run.json",
            &json!({
                "play": report.play, "wake_at_start": report.wake_at_start,
                "terminal": format!("{:?}", report.terminal), "failure": report.failure,
                "kernel_failure": report.kernel_failure.map(|failure| format!("{failure:?}")),
                "cleanup_failure": report.cleanup_failure,
                "partition_identity_maps": report.partitions.iter().map(|part| format!("{part:#?}")).collect::<Vec<_>>(),
                "host_requests": report.requests.iter().map(|request| format!("{request:?}")).collect::<Vec<_>>(),
                "kernel_events": report.kernel_events.iter().map(|event| json!({
                    "node": event.node.0, "port": event.port.map(|port| port.0),
                    "kind": format!("{:?}",event.kind), "sequence": event.sequence,
                    "request": event.request.map(|request| request.0),
                })).collect::<Vec<_>>(),
            }),
        )?;
        artifacts.push("body-run.json".into());
    }
    let causal_export = causal(output, plan, report)?;
    artifacts.push("causal-inspection.json".into());
    let debugger_export = match report {
        Some(report) => match super::debugger::export(output, plan, report) {
            Ok(summary) => {
                artifacts.extend([
                    "debugger-timeline.json".into(),
                    "debugger-upstream-trace.json".into(),
                ]);
                serde_json::to_value(summary)?
            }
            Err(error) => json!({"refusal": error.to_string(), "rendered_browser_verified": false}),
        },
        None => json!({"refusal":"no Body run report", "rendered_browser_verified":false}),
    };
    let final_response_causal_export = match report {
        Some(report) => match super::causal_chain::export(output, plan, report) {
            Ok(summary) => {
                artifacts.push("causal-final-response.json".into());
                serde_json::to_value(summary)?
            }
            Err(error) => json!({"refusal":error.to_string()}),
        },
        None => json!({"refusal":"no Body run report"}),
    };
    let state = capture
        .states
        .last()
        .and_then(|bytes| conduit_human::ExperienceAutobiographyState::decode(bytes).ok());
    let receipt = Receipt {
        schema: "conduit.pete/live-cognition-proof@1",
        proof_class: "live-local-ollama-actual-body-play",
        input_class: "scripted typed observations; not physical sensor proof",
        accepted: failure.is_none() && criteria.iter().all(|criterion| criterion.passed),
        body_id: plan.body_id.as_str().into(),
        wake_id: plan.wake_id.as_str().into(),
        plan_id: plan.plan_id.as_str().into(),
        active_play_id: report.map(|report| report.play.active_play_id.as_str().into()),
        model_content_identity: model.identity.model_content_identity.clone(),
        model_calls: provider.calls,
        responses: capture.responses.len(),
        completed: state.as_ref().map(|state| *state.completed()),
        retained_records: state
            .as_ref()
            .map_or(0, |state| state.records().get().as_slice().len()),
        clock_waits: clock.waits,
        clock_waits_after_feedback: clock.after_feedback,
        clock_ticks: String::from_utf8_lossy(clock_output)
            .matches("Clock tick")
            .count(),
        timed_out: clock.timed_out,
        failure,
        criteria,
        causal_export,
        debugger_export,
        final_response_causal_export,
        artifacts,
    };
    json_file(output, "receipt.json", &receipt)?;
    Ok(receipt)
}
fn causal(
    output: &Path,
    plan: &BodyPlan,
    report: Option<&BodyRunReport>,
) -> Result<String, Box<dyn Error>> {
    let record = report.map(|report| {
        conduit_std_host::body_causal_evidence::BodyRunCausalRecord::from_run(plan, report)
    });
    let (status, value) = match record {
        Some(Ok(record)) => {
            let traces = record.retained_evidence().map(|identity| {
                match conduit_observatory::explain_trace_with_metadata(record.graph(), &record, identity,
                    conduit_observatory::CausalExplanationVisibility::Operator) {
                    Ok(trace) => json!({"evidence": format!("{identity:?}"), "summary":trace.summary,
                        "completeness":format!("{:?}", trace.completeness),
                        "nodes":trace.nodes.iter().map(|node| format!("{node:?}")).collect::<Vec<_>>(),
                        "edges":trace.edges.iter().map(|edge| json!({"effect":edge.effect,
                            "relationship":format!("{:?}",edge.relationship), "cause":edge.cause})).collect::<Vec<_>>()}),
                    Err(error) => json!({"evidence":format!("{identity:?}"),"refusal":format!("{error:?}")}),
                }
            }).collect::<Vec<_>>();
            (
                format!("actual bounded Body causal inspection exported: {} traces, {} refusals; no rendered Patchbay proof", traces.len(), traces.iter().filter(|trace| trace.get("refusal").is_some()).count()),
                json!({"schema":"conduit.pete/causal-inspection@1",
                "projection":"actual retained causal evidence; not fabricated debugger timeline", "traces":traces}),
            )
        }
        Some(Err(error)) => (
            format!("bounded causal projection refused: {error:?}"),
            json!({"refusal":format!("{error:?}")}),
        ),
        None => (
            "Body did not produce a run report".into(),
            json!({"refusal":"no run report"}),
        ),
    };
    if let Err(error) = json_file(output, "causal-inspection.json", &value) {
        let status = format!("causal export refused: {error}");
        json_file(
            output,
            "causal-inspection.json",
            &json!({"refusal": status}),
        )?;
        return Ok(status);
    }
    Ok(status)
}
fn json_file(path: &Path, name: &str, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(format!("{name} exceeds proof's finite JSON artifact bound").into());
    }
    fs::write(path.join(name), bytes)?;
    Ok(())
}
