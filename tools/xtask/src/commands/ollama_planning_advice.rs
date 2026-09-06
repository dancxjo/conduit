use crate::{
    cli::{GlobalOpts, ProveArgs},
    process::StepError,
};
use conduit_core::{BaseImplementationId, BootId, CapabilityId, HostId, OfferGeneration};
use conduit_form::parse_with_startup;
use conduit_planner::{
    default_placements, plan, seed_planning_from_advice, PlanningAdvice, SuggestedPlacement,
};
use conduit_signal::signal_profile_catalog;
use conduit_signal_conformance::pico_local_advertisement;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

const PROOF_ID: &str = "prove.llm-planning-advice";
const DEFAULT_OLLAMA_URL: &str = "http://forebrain.local:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gpt-oss:20b";
const MAXIMUM_REQUEST_BYTES: usize = 16 * 1024;
const MAXIMUM_RESPONSE_BYTES: usize = 64 * 1024;
const MAXIMUM_PROPOSAL_BYTES: usize = 8 * 1024;
const MAXIMUM_MODEL_ID_BYTES: usize = 128;

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    message: OllamaMessage,
    done: bool,
    done_reason: Option<String>,
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireProposal {
    proposal_id: String,
    request_identity: String,
    run_identity: String,
    checked_form_id: String,
    placements: Vec<WirePlacement>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WirePlacement {
    gear_id: String,
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    capability_id: String,
}

#[derive(Serialize)]
struct LiveProof {
    schema_version: u16,
    proof_class: &'static str,
    ollama_version: String,
    model: String,
    endpoint: String,
    git_head: String,
    request_sha256: String,
    response_sha256: String,
    proposal_sha256: String,
    untrusted_model_proposal: WireProposal,
    model_proposal_is_advisory: bool,
    proposal_id: String,
    request_identity: String,
    run_identity: String,
    checked_form_id: String,
    selected_host_id: String,
    selected_boot_id: String,
    selected_offer_generation: u64,
    selected_capability_id: String,
    baseline_plan_id: String,
    advised_plan_id: String,
    ordinary_planner_revalidated: bool,
    proposal_did_not_mint_plan_identity: bool,
    active_plan_mutated: bool,
    model_output_is_sign_evidence: bool,
    done_reason: Option<String>,
    eval_count: Option<u64>,
    success: bool,
}

pub fn run(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    let endpoint = args.ollama_url.as_deref().unwrap_or(DEFAULT_OLLAMA_URL);
    let model = args.ollama_model.as_deref().unwrap_or(DEFAULT_OLLAMA_MODEL);
    validate_configuration(endpoint, model)?;
    if opts.dry_run {
        println!(
            "llm-planning-advice: would request one bounded typed proposal from {model} at {endpoint} and submit it to the ordinary planner"
        );
        return Ok(());
    }

    let (form, hosts) = fixture();
    let ordinary_choices = default_placements(&form, &hosts)
        .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    let baseline = plan(
        &form,
        &hosts,
        &ordinary_choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    let sealed_baseline = baseline.clone();

    let request = request(model, &form, &hosts)?;
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    if request_bytes.len() > MAXIMUM_REQUEST_BYTES {
        return Err(StepError::prereq(
            PROOF_ID,
            "Ollama request exceeds its finite bound",
        ));
    }
    let response_bytes = post_with_curl(endpoint, &request_bytes)?;
    let response: OllamaResponse = serde_json::from_slice(&response_bytes).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            "Ollama response is not the expected bounded envelope",
        )
    })?;
    if !response.done || response.model != model || response.message.content.is_empty() {
        return Err(StepError::prereq(
            PROOF_ID,
            "Ollama did not complete one exact non-empty model proposal",
        ));
    }
    if response.message.content.len() > MAXIMUM_PROPOSAL_BYTES {
        return Err(StepError::prereq(
            PROOF_ID,
            "model proposal exceeds its finite byte bound",
        ));
    }
    let wire: WireProposal = serde_json::from_str(&response.message.content).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            "model output is not the exact typed planning-advice schema",
        )
    })?;
    let advice = convert(wire.clone())?;
    let seeded = seed_planning_from_advice(&form, &hosts, &[], &advice).map_err(|error| {
        StepError::prereq(PROOF_ID, format!("planning advice refused: {error:?}"))
    })?;
    let advised = plan(
        &form,
        &hosts,
        &seeded.placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    if baseline != sealed_baseline || baseline.plan_id == advised.plan_id {
        return Err(StepError::prereq(
            PROOF_ID,
            "live advice either mutated prior Plan truth or did not select a distinct realization",
        ));
    }
    let suggestion = advice
        .placements
        .first()
        .ok_or_else(|| StepError::prereq(PROOF_ID, "model proposed no placement"))?;
    if suggestion.host_id != hosts[1].host_id {
        return Err(StepError::prereq(
            PROOF_ID,
            "model did not choose the exact lower-resource candidate",
        ));
    }

    let proof = LiveProof {
        schema_version: 2,
        proof_class: "live-local-model",
        ollama_version: ollama_version(endpoint)?,
        model: response.model,
        endpoint: endpoint.to_string(),
        git_head: git_head(root)?,
        request_sha256: hex_digest(&request_bytes),
        response_sha256: hex_digest(&response_bytes),
        proposal_sha256: hex_digest(response.message.content.as_bytes()),
        untrusted_model_proposal: wire,
        model_proposal_is_advisory: true,
        proposal_id: seeded.evidence.proposal_id,
        request_identity: seeded.evidence.request_identity,
        run_identity: seeded.evidence.run_identity,
        checked_form_id: seeded.evidence.checked_form_id.as_str().to_string(),
        selected_host_id: suggestion.host_id.as_str().to_string(),
        selected_boot_id: suggestion.boot_id.as_str().to_string(),
        selected_offer_generation: suggestion.offer_generation.0,
        selected_capability_id: suggestion.capability_id.as_str().to_string(),
        baseline_plan_id: baseline.plan_id.as_str().to_string(),
        advised_plan_id: advised.plan_id.as_str().to_string(),
        ordinary_planner_revalidated: true,
        proposal_did_not_mint_plan_identity: true,
        active_plan_mutated: false,
        model_output_is_sign_evidence: false,
        done_reason: response.done_reason,
        eval_count: response.eval_count,
        success: true,
    };
    let path = root.join("target/ollama-planning-advice-live.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?,
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    println!(
        "llm-planning-advice: {model} proposed {} and ordinary planning sealed {} ({})",
        suggestion.host_id.as_str(),
        advised.plan_id.as_str(),
        path.display()
    );
    Ok(())
}

fn fixture() -> (
    conduit_form::CheckedForm,
    [conduit_core::HostAdvertisement; 2],
) {
    let form = parse_with_startup(
        "form advised {\n    pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("reviewed planning-advice fixture checks");
    let mut first = pico_local_advertisement();
    first.host_id = HostId::from("advice/host-a");
    first.boot_id = BootId::from("advice/boot-a");
    first.offer_generation = OfferGeneration(1);
    first.capabilities[0].capability_id = CapabilityId::from("advice/pulse-a");
    first.capabilities[0].resource_requirements[0].units = 3;
    let resource_class = first.capabilities[0].resource_requirements[0]
        .class_id
        .clone();
    first
        .resources
        .iter_mut()
        .find(|pool| pool.class_id == resource_class)
        .expect("reviewed fixture has the required resource pool")
        .capacity_units = 3;
    let mut second = first.clone();
    second.host_id = HostId::from("advice/host-b");
    second.boot_id = BootId::from("advice/boot-b");
    second.offer_generation = OfferGeneration(7);
    second.capabilities[0].capability_id = CapabilityId::from("advice/pulse-b");
    second.capabilities[0].resource_requirements[0].units = 1;
    second
        .resources
        .iter_mut()
        .find(|pool| pool.class_id == resource_class)
        .expect("reviewed fixture has the required resource pool")
        .capacity_units = 1;
    (form, [first, second])
}

fn request(
    model: &str,
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement; 2],
) -> Result<serde_json::Value, StepError> {
    let candidates = hosts
        .iter()
        .map(|host| {
            let offer = &host.capabilities[0];
            serde_json::json!({
                "host_id": host.host_id.as_str(),
                "boot_id": host.boot_id.as_str(),
                "offer_generation": host.offer_generation.0,
                "capability_id": offer.capability_id.as_str(),
                "resource_units": offer.resource_requirements[0].units,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "model": model,
        "stream": false,
        "think": false,
        "format": proposal_schema(),
        "messages": [
            {"role": "system", "content": "You are an optional Conduit planning adviser. Return only the requested typed proposal. Never invent identifiers. You do not create a Plan. Prefer the candidate with lower resource units."},
            {"role": "user", "content": format!(
                "Checked Form: {}. Gear: advised/pulse. Exact candidates: {}. Use proposal_id=proposal/live-gpt-oss request_identity=request/live-gpt-oss run_identity=run/live-gpt-oss.",
                form.checked_form_id.as_str(),
                serde_json::to_string(&candidates).map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?,
            )}
        ],
        "options": {"temperature": 0, "num_predict": 512}
    }))
}

fn proposal_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "proposal_id": {"type": "string"},
            "request_identity": {"type": "string"},
            "run_identity": {"type": "string"},
            "checked_form_id": {"type": "string"},
            "placements": {"type": "array", "maxItems": 1, "items": {
                "type": "object",
                "properties": {
                    "gear_id": {"type": "string"}, "host_id": {"type": "string"},
                    "boot_id": {"type": "string"}, "offer_generation": {"type": "integer"},
                    "capability_id": {"type": "string"}
                },
                "required": ["gear_id", "host_id", "boot_id", "offer_generation", "capability_id"],
                "additionalProperties": false
            }}
        },
        "required": ["proposal_id", "request_identity", "run_identity", "checked_form_id", "placements"],
        "additionalProperties": false
    })
}

fn convert(wire: WireProposal) -> Result<PlanningAdvice, StepError> {
    if wire.placements.len() != 1 {
        return Err(StepError::prereq(
            PROOF_ID,
            "live comparison requires exactly one suggested placement",
        ));
    }
    Ok(PlanningAdvice {
        proposal_id: wire.proposal_id,
        request_identity: wire.request_identity,
        run_identity: wire.run_identity,
        checked_form_id: conduit_core::CheckedFormId::from(wire.checked_form_id),
        placements: wire
            .placements
            .into_iter()
            .map(|placement| SuggestedPlacement {
                gear_id: conduit_core::GearId::from(placement.gear_id),
                host_id: HostId::from(placement.host_id),
                boot_id: BootId::from(placement.boot_id),
                offer_generation: OfferGeneration(placement.offer_generation),
                capability_id: CapabilityId::from(placement.capability_id),
            })
            .collect(),
        lines: vec![],
    })
}

fn post_with_curl(endpoint: &str, body: &[u8]) -> Result<Vec<u8>, StepError> {
    let url = format!("{}/api/chat", endpoint.trim_end_matches('/'));
    let mut child = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "180",
            "--max-filesize",
            "65536",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            &url,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| StepError::prereq(PROOF_ID, format!("cannot start curl: {error}")))?;
    child
        .stdin
        .take()
        .ok_or_else(|| StepError::prereq(PROOF_ID, "curl stdin unavailable"))?
        .write_all(body)
        .map_err(|error| {
            StepError::prereq(PROOF_ID, format!("cannot write Ollama request: {error}"))
        })?;
    let output = child
        .wait_with_output()
        .map_err(|error| StepError::prereq(PROOF_ID, format!("cannot wait for Ollama: {error}")))?;
    if !output.status.success() {
        return Err(StepError::prereq(
            PROOF_ID,
            format!(
                "Ollama request failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    if output.stdout.is_empty() || output.stdout.len() > MAXIMUM_RESPONSE_BYTES {
        return Err(StepError::prereq(
            PROOF_ID,
            "Ollama response violates its byte bound",
        ));
    }
    Ok(output.stdout)
}

fn ollama_version(endpoint: &str) -> Result<String, StepError> {
    let url = format!("{}/api/version", endpoint.trim_end_matches('/'));
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            &url,
        ])
        .output()
        .map_err(|error| {
            StepError::prereq(PROOF_ID, format!("cannot query Ollama version: {error}"))
        })?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| StepError::prereq(PROOF_ID, "Ollama version response is malformed"))?;
    value["version"]
        .as_str()
        .filter(|version| !version.is_empty())
        .map(str::to_string)
        .ok_or_else(|| StepError::prereq(PROOF_ID, "Ollama version is absent"))
}

fn validate_configuration(endpoint: &str, model: &str) -> Result<(), StepError> {
    if !endpoint.starts_with("http://")
        || endpoint.len() > 512
        || endpoint.contains('@')
        || endpoint.contains(['\r', '\n'])
    {
        return Err(StepError::prereq(
            PROOF_ID,
            "Ollama endpoint must be bounded credential-free HTTP",
        ));
    }
    if model.is_empty() || model.len() > MAXIMUM_MODEL_ID_BYTES || model.contains(['\r', '\n']) {
        return Err(StepError::prereq(
            PROOF_ID,
            "Ollama model identity is invalid",
        ));
    }
    Ok(())
}

fn git_head(root: &Path) -> Result<String, StepError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    if !output.status.success() {
        return Err(StepError::prereq(PROOF_ID, "cannot resolve exact Git head"));
    }
    String::from_utf8(output.stdout)
        .map(|head| head.trim().to_string())
        .map_err(|_| StepError::prereq(PROOF_ID, "Git head is not UTF-8"))
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "ollama_planning_advice_tests.rs"]
mod tests;
