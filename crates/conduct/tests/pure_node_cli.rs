use std::path::PathBuf;
use std::process::Command;

use sha2::{Digest as _, Sha256};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn canonical_cli_matches_the_pure_node_value_and_normalized_evidence() {
    let profile: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/pure-node-v1-profile.json"
    ))
    .unwrap();
    let panel = workspace_file("conformance/c5/pure-node-v1.panel");

    let human = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .arg(&panel)
        .output()
        .unwrap();
    assert!(human.status.success(), "{:?}", human.stderr);
    assert_eq!(
        String::from_utf8(human.stdout).unwrap(),
        profile["expected_stdout_utf8"].as_str().unwrap()
    );

    let ndjson = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .args(["--format", "ndjson"])
        .arg(&panel)
        .output()
        .unwrap();
    assert!(ndjson.status.success(), "{:?}", ndjson.stderr);
    let records = String::from_utf8(ndjson.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(records.iter().any(|record| {
        record["record"] == "summary"
            && record["nodes_completed"] == 3
            && record["cords_conducted"] == 2
    }));
    let stdout = records
        .iter()
        .filter(|record| record["record"] == "channel_chunk" && record["channel"] == "stdout")
        .flat_map(|record| {
            record["payload_hex"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        profile["expected_stdout_utf8"].as_str().unwrap()
    );

    let mut evidence = records
        .into_iter()
        .filter(|record| record["record"] == "exact_execution_evidence")
        .map(|record| record["evidence"].clone())
        .collect::<Vec<_>>();
    assert!(!evidence.is_empty());
    for event in &mut evidence {
        event["run_id"] = serde_json::Value::String("<normalized-run>".to_owned());
        if event["subject_kind"] == "run" {
            event["subject_id"] = serde_json::Value::String("<normalized-run>".to_owned());
        }
    }
    let identity = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&evidence).unwrap())
    );
    assert_eq!(
        identity,
        profile["normalized_evidence_sha256"].as_str().unwrap()
    );
    assert!(evidence.iter().any(|event| {
        event["event_kind"] == "terminal" && event["terminal_cause"] == "succeeded"
    }));
}
