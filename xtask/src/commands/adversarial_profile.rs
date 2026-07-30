use std::{fs, path::Path};

use serde_json::{Value, json};

pub fn run(workspace_root: &Path, profile: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_path = workspace_root.join("conformance/c5/adversarial-containment-v1.json");
    if !fixture_path.exists() {
        return Err(format!("Fixture missing: {}", fixture_path.display()).into());
    }

    let fixture: Value = serde_json::from_str(&fs::read_to_string(&fixture_path)?)?;
    let constrained_set = [
        "budget-reset-across-lifecycle",
        "old-hazardous-command-after-transition",
    ];

    let cases_arr = fixture
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("cases missing in fixture")?;

    let mut cases = Vec::new();
    for case in cases_arr {
        let id = case.get("id").and_then(Value::as_str).unwrap_or("");
        let status = if profile == "constrained" && constrained_set.contains(&id) {
            "executed-by-conduit-embedded-test"
        } else {
            "unsupported"
        };
        let reason = if status.starts_with("executed") {
            Value::Null
        } else {
            json!("profile has no production implementation or physical fixture for this attack")
        };

        cases.push(json!({
            "id": id,
            "status": status,
            "reason": reason,
        }));
    }

    let report = json!({
        "schema": "conduit.adversarial-profile-report/v1",
        "profile": profile,
        "fixture_seed": fixture.get("seed"),
        "cases": cases,
        "claim_boundary": fixture.get("claim_boundary"),
    });

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
