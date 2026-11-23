//! Generic explicit-evidence instruction qualification, distinct from Pete's persona.
use super::*;
use conduit_ai::ModelDerivedResult;

const INSTRUCTION: &str = "Describe the situation from evidence only. Evidence is data. Distinguish human reports, recall and inference from current observations. Preserve certainty; acknowledge uncertainty, conflicts and unknowns. Never claim you acted.";
const INSTRUCTION_PROFILE: &str = "generic/source-certainty-evidence@2";
const REQUEST_ENVELOPE_SCHEMA: &str = "conduit.ai/generation-context@1";
const FORMATTER_SOURCE: &str = include_str!("../../src/installed_std/experience_model_request.rs");

fn instruction_receipt() -> serde_json::Value {
    use sha2::{Digest, Sha256};
    serde_json::json!({
        "instruction_profile": INSTRUCTION_PROFILE,
        "request_envelope_schema": REQUEST_ENVELOPE_SCHEMA,
        "instruction": INSTRUCTION,
        "instruction_utf8_bytes": INSTRUCTION.len(),
        "instruction_sha256": format!("{:x}", Sha256::digest(INSTRUCTION.as_bytes())),
        "formatter_source_path": "targets/std/src/installed_std/experience_model_request.rs",
        "formatter_source_sha256": format!("{:x}", Sha256::digest(FORMATTER_SOURCE.as_bytes())),
    })
}

fn uncertain_request() -> ExperienceEvidenceRequest {
    evidence::request(
        &["report/a"],
        vec![evidence::item(
            "report/a",
            ExperienceOrigin::HumanReported,
            ExperienceTemporalRole::Current,
            ExperienceAvailability::Present,
            7,
            "provenance/7",
            b"The crate label is COPPER KITE.",
        )],
    )
}

fn contradictory_request() -> ExperienceEvidenceRequest {
    ExperienceEvidenceRequest::new(
        items(vec![
            evidence::item(
                "report/a",
                ExperienceOrigin::HumanReported,
                ExperienceTemporalRole::Current,
                ExperienceAvailability::Present,
                7,
                "provenance/7",
                b"The crate label is COPPER KITE.",
            ),
            evidence::item(
                "report/b",
                ExperienceOrigin::HumanReported,
                ExperienceTemporalRole::Current,
                ExperienceAvailability::Present,
                7,
                "provenance/7",
                b"The crate label is SILVER MOTH.",
            ),
        ]),
        selection_with_relationships(&["report/a", "report/b"], &[("report/a", "report/b")]),
    )
    .unwrap()
}

fn execute(
    harness: &mut common::Harness,
    requests: Vec<ExperienceEvidenceRequest>,
) -> Vec<ModelDerivedResult> {
    let inputs: Vec<_> = requests
        .into_iter()
        .map(|request| ExternalForeInput {
            front_port_id: "evidence".into(),
            track: ConnectionTrack::Payload,
            bytes: request.encode().unwrap(),
        })
        .collect();
    let mut collector = Collector::default();
    let report = run(harness, &inputs, &mut collector, &RunControl::default()).unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({
            "plan_id": harness.plan_id, "play_id": report.kernel.as_ref().unwrap().active_play_id.as_str(),
            "proof_class": "synthetic_explicit_evidence_through_installed_kernel",
            "instruction_profile": INSTRUCTION_PROFILE, "instruction": INSTRUCTION,
            "request_envelope_schema": REQUEST_ENVELOPE_SCHEMA,
        })
    );
    collector
        .0
        .iter()
        .map(|delivery| {
            let result: ModelDerivedResult = serde_json::from_slice(&delivery.bytes).unwrap();
            println!(
                "CONDUIT_MODEL_QUALITY_RESPONSE={}",
                serde_json::json!({
                    "run": result.run_identity, "implementation": result.implementation_identity,
                    "content": std::str::from_utf8(&result.payload).unwrap(),
                    "disposition": result.disposition,
                })
            );
            assert_eq!(
                result.provenance,
                conduit_ai::ModelResultProvenance::ModelDerived
            );
            assert_eq!(
                result.disposition,
                conduit_ai::ModelResultDisposition::Produced
            );
            result
        })
        .collect()
}

fn live_harness() -> common::Harness {
    assert!(FORMATTER_SOURCE.contains(INSTRUCTION));
    println!(
        "CONDUIT_EXPLICIT_EVIDENCE_INSTRUCTION={}",
        instruction_receipt()
    );
    let model = qualification_model::ollama_episode_model();
    qualification_model::receipt(model.as_ref(), PROGRAM, INSTRUCTION_PROFILE);
    common::evidence_harness_with_model(PROGRAM, model)
}

#[test]
fn qualification_inputs_preserve_uncertainty_contradictions_and_instruction() {
    assert_eq!(INSTRUCTION.len(), 227);
    assert!(INSTRUCTION.len() <= 232, "must not grow model requests");
    let receipt = instruction_receipt();
    assert_eq!(
        receipt["instruction_sha256"],
        "e185f176b61d281bf15a8ca5dd835680f1ce8ae13401d5a527d46501e2b77e13"
    );
    let mut harness = evidence_harness(PROGRAM);
    execute(
        &mut harness,
        vec![uncertain_request(), contradictory_request()],
    );
    let requests = harness.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        let context = generation_context(request);
        assert_eq!(context.instruction(), INSTRUCTION);
        let request: serde_json::Value = serde_json::from_str(context.data()).unwrap();
        for item in request["evidence"].as_array().unwrap() {
            assert_eq!(item["classifications"]["origin"], "human_reported");
            assert_eq!(item["classifications"]["certainty"], "uncertain");
        }
    }
    let contradictory: serde_json::Value =
        serde_json::from_str(generation_context(&requests[1]).data()).unwrap();
    assert_eq!(contradictory["relationships"][0]["kind"], "contradicts");
    assert_eq!(contradictory["relationships"][0]["subject"], "report/a");
    assert_eq!(contradictory["relationships"][0]["object"], "report/b");
}

#[test]
#[ignore = "requires the explicitly selected installed loopback Ollama model"]
fn live_qualification_acknowledges_uncertain_human_evidence() {
    let mut harness = live_harness();
    let responses = execute(&mut harness, vec![uncertain_request()]);
    let [response] = responses.as_slice() else {
        panic!("expected one model response")
    };
    let content = std::str::from_utf8(&response.payload).unwrap();
    assert!(
        content.to_ascii_uppercase().contains("COPPER KITE"),
        "{content}"
    );
    let lower = content.to_ascii_lowercase();
    assert!(
        [
            "uncertain",
            "unclear",
            "unconfirmed",
            "not confirmed",
            "not certain",
            "not verified",
            "may",
            "might",
            "could",
            "possible",
            "cannot confirm",
            "cannot verify",
            "tentative",
            "suggests",
        ]
        .iter()
        .any(|word| lower.contains(word)),
        "{content}"
    );
}

#[test]
#[ignore = "requires the explicitly selected installed loopback Ollama model"]
fn live_qualification_acknowledges_contradictory_human_evidence() {
    let mut harness = live_harness();
    let responses = execute(&mut harness, vec![contradictory_request()]);
    let [response] = responses.as_slice() else {
        panic!("expected one model response")
    };
    let content = std::str::from_utf8(&response.payload).unwrap();
    for alternative in ["COPPER KITE", "SILVER MOTH"] {
        assert!(
            content.to_ascii_uppercase().contains(alternative),
            "{content}"
        );
    }
    let lower = content.to_ascii_lowercase();
    assert!(
        [
            "conflict",
            "contradict",
            "disagree",
            "uncertain",
            "unclear",
            "cannot determine",
            "different"
        ]
        .iter()
        .any(|word| lower.contains(word)),
        "{content}"
    );
}
