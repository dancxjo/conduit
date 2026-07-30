use std::fs;
use std::path::PathBuf;

use conduit_compile::{InstalledProfile, compile_source};
use conduit_web::{cancel_panel, run_panel};
use sha2::{Digest as _, Sha256};

const SOURCE: &str = include_str!("../../../conformance/c5/pure-node-v1.panel");
const PROFILE: &str = include_str!("../../../conformance/c5/pure-node-v1-profile.json");

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn checked_pure_node_plan_and_browser_run_are_exact_and_bounded() {
    let profile: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    let installed = InstalledProfile::observe(SOURCE).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let actual_plan = serde_json::to_value(&document).unwrap();

    if std::env::var_os("CONDUIT_PRINT_PURE_NODE_PLAN").is_some() {
        println!("{}", serde_json::to_string(&actual_plan).unwrap());
        return;
    }

    let golden: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace_file("conformance/c5/pure-node-v1-plan.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(actual_plan, golden, "exact plan golden drifted");

    let result: serde_json::Value = serde_json::from_str(&run_panel(SOURCE.to_owned())).unwrap();
    let mut normalized_evidence = result["evidence"].clone();
    for event in normalized_evidence.as_array_mut().unwrap() {
        event["run_id"] = serde_json::Value::String("<normalized-run>".to_owned());
        if event["subject_kind"] == "run" {
            event["subject_id"] = serde_json::Value::String("<normalized-run>".to_owned());
        }
    }
    let evidence_identity = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&normalized_evidence).unwrap())
    );
    if std::env::var_os("CONDUIT_PRINT_PURE_NODE_EVIDENCE").is_some() {
        println!("{evidence_identity}");
        return;
    }
    assert_eq!(
        evidence_identity, profile["normalized_evidence_sha256"],
        "normalized browser evidence identity drifted"
    );
    assert_eq!(result["ok"], true);
    assert_eq!(result["terminal"], "succeeded");
    assert_eq!(result["profile"], "exact-plan-deterministic-executor");
    assert_eq!(
        result["patchbay"]["source"]["semantic_hash"],
        result["patchbay"]["semantic"]["source_semantic_hash"]
    );
    assert_eq!(
        result["patchbay"]["plan"]["identity"],
        result["patchbay"]["run"]["plan_identity"]
    );
    assert_eq!(
        result["patchbay"]["plan"]["source_semantic_hash"],
        result["patchbay"]["run"]["source_semantic_hash"]
    );
    assert_eq!(result["patchbay"]["evidence"], result["evidence"]);
    assert_ne!(
        result["patchbay"]["presentation"]["identity"],
        result["patchbay"]["plan"]["identity"]
    );
    assert_eq!(
        result["stdout"], profile["expected_stdout_utf8"],
        "browser typed value drifted"
    );
    assert!(
        result["high_water"]["queue_items"].as_u64().unwrap()
            <= profile["bounds"]["queue_items"].as_u64().unwrap()
    );
    assert!(
        result["high_water"]["queue_payload_bytes"]
            .as_u64()
            .unwrap()
            <= profile["bounds"]["queue_payload_bytes"].as_u64().unwrap()
    );
    assert!(
        result["scheduler_event_count"].as_u64().unwrap()
            <= profile["bounds"]["scheduler_events"].as_u64().unwrap()
    );
    assert!(
        result["evidence_bytes"].as_u64().unwrap()
            <= profile["bounds"]["evidence_bytes"].as_u64().unwrap()
    );
}

#[test]
fn pure_node_stdout_requires_an_active_exact_grant() {
    let denied = InstalledProfile::observe_with_stdout_grant(SOURCE, false).unwrap();
    let error = compile_source(SOURCE, &denied.input).unwrap_err();
    assert_eq!(error.code(), "CND-CMP-006");
}

#[test]
fn pure_node_uppercase_requires_an_installed_candidate() {
    let installed = InstalledProfile::observe(SOURCE).unwrap();
    let mut unavailable = installed.input;
    unavailable
        .candidates
        .retain(|candidate| candidate.implementation.id != "conduit/hosted-uppercase-v1");
    unavailable.seal().unwrap();
    let error = compile_source(SOURCE, &unavailable).unwrap_err();
    assert_eq!(error.code(), "CND-CMP-005");
}

#[test]
fn pure_node_oversized_value_fails_closed() {
    let oversized = SOURCE.replace(
        "Conduit exact slice.\\n",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    let result: serde_json::Value = serde_json::from_str(&run_panel(oversized)).unwrap();
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["code"], "CND-SCH-007", "{result}");
}

#[test]
fn pure_node_insufficient_queue_budget_fails_before_start() {
    let underfunded = SOURCE.replacen("max_queued_bytes = 64", "max_queued_bytes = 32", 1);
    let result: serde_json::Value = serde_json::from_str(&run_panel(underfunded)).unwrap();
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["code"], "CND-FLW-003", "{result}");
}

#[test]
fn pure_node_abort_has_one_deterministic_terminal_record() {
    let result: serde_json::Value = serde_json::from_str(&cancel_panel(SOURCE.to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["terminal"], "cancelled");
    assert_eq!(result["stdout"], "");
    let terminal = result["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["event_kind"] == "terminal")
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 1, "{result}");
    assert_eq!(terminal[0]["terminal_cause"], "cancelled");
}
