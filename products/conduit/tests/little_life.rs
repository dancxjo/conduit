use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

#[test]
fn canonical_little_life_completes_exactly_thirty_two_presented_generations() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = workspace.join(format!(
        "target/little-life-form-proof-{}.json",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .current_dir(&workspace)
        .args([
            "run",
            "forms/little-life/main.conduit",
            "--await-terminal",
            "--report",
        ])
        .arg(&report)
        .output()
        .expect("run canonical Little Life through the Conduit product entrance");
    assert!(
        output.status.success(),
        "Little Life failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let transcript = String::from_utf8(output.stdout).expect("presentation is UTF-8");
    let generations = transcript
        .lines()
        .filter_map(|line| {
            line.strip_prefix("SCALAR-FIELD title=\"Orbium evolution\" ")?
                .split_whitespace()
                .find_map(|part| part.strip_prefix("generation="))?
                .parse::<u64>()
                .ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(generations, (1..=32).collect::<Vec<_>>());

    let document: Value =
        serde_json::from_slice(&std::fs::read(&report).expect("read Little Life execution report"))
            .expect("decode Little Life execution report");
    std::fs::remove_file(&report).expect("remove bounded Little Life test report");
    let plans = document["plans"].as_array().expect("report retains Plans");
    assert_eq!(plans.len(), 1);
    let plan_id = plans[0]["plan_id"].as_str().expect("Plan has identity");
    let terminal = document["observations"]
        .as_array()
        .expect("report retains observations")
        .iter()
        .find(|item| item.pointer("/kind/PlanTerminal").is_some())
        .expect("report retains a Plan terminal");
    assert_eq!(
        terminal.pointer("/kind/PlanTerminal/disposition"),
        Some(&Value::String("Completed".into()))
    );
    let play_id = terminal["active_play_id"]
        .as_str()
        .expect("terminal has active Play identity");
    println!("{EVIDENCE_MARKER}{{\"plan_id\":\"{plan_id}\",\"play_id\":\"{play_id}\"}}");
}
