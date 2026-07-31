use std::path::PathBuf;
use std::process::Command;

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn canonical_cli_matches_the_pure_node_value_and_normalized_evidence() {
    let profile: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/pure-node-profile.json"
    ))
    .unwrap();
    let panel = workspace_file("conformance/c5/pure-node.panel");

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
            && record["nodes_completed"] == 4
            && record["cords_conducted"] == 3
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

    let evidence = records
        .into_iter()
        .filter(|record| record["record"] == "exact_execution_evidence")
        .map(|record| record["evidence"].clone())
        .collect::<Vec<_>>();
    assert!(!evidence.is_empty());
    let required_evidence_fields = profile["required_evidence_fields"].as_array().unwrap();
    for event in &evidence {
        for field in required_evidence_fields {
            assert!(
                event.get(field.as_str().unwrap()).is_some(),
                "evidence event omitted required `{field}`: {event}"
            );
        }
    }
    assert!(evidence.iter().any(|event| {
        event["event_kind"] == "terminal" && event["terminal_cause"] == "succeeded"
    }));
}
