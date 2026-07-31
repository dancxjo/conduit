use std::{fs, path::Path};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn run(workspace_root: &Path, check: bool) -> Result<(), Box<dyn std::error::Error>> {
    let output_path = workspace_root.join("tour/public/browser-plan.json");

    let artifacts_def = [
        (
            "browser-host-adapter",
            workspace_root.join("browser/conduit-browser-host.mjs"),
            "../../browser/conduit-browser-host.mjs",
        ),
        (
            "tour-worker",
            workspace_root.join("tour/public/tour-worker.mjs"),
            "./tour-worker.mjs",
        ),
        (
            "wasm-bindgen-loader",
            workspace_root.join("tour/public/conduit_web.js"),
            "./conduit_web.js",
        ),
        (
            "conduit-web-wasm",
            workspace_root.join("tour/public/conduit_web_bg.wasm"),
            "./conduit_web_bg.wasm",
        ),
    ];

    let mut artifacts = Vec::new();
    for (id, path, public_path) in artifacts_def {
        if !path.exists() {
            return Err(format!("Artifact file missing: {}", path.display()).into());
        }
        let bytes = fs::read(&path)?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let size = bytes.len();
        artifacts.push(json!({
            "id": id,
            "path": public_path,
            "sha256": sha256,
            "bytes": size
        }));
    }

    let identity_input = json!({
        "schema": "conduit.tour-browser-plan",
        "implementation_id": "conduit/tour-production-wasm-worker",
        "semantic_contract": "conduit/tour-panel-run",
        "placement": "dedicated-worker",
        "artifacts": artifacts
    });

    let identity_bytes = serde_json::to_vec(&identity_input)?;
    let plan_identity = format!("{:x}", Sha256::digest(&identity_bytes));

    let mut plan = identity_input;
    if let Value::Object(ref mut map) = plan {
        map.insert("plan_identity".to_string(), json!(plan_identity));
        map.insert(
            "observation_id".to_string(),
            json!("conduit/tour-static-browser-observation"),
        );
        map.insert(
            "bounds".to_string(),
            json!({
                "maximum_pending": 1,
                "maximum_message_bytes": 131072,
                "response_timeout_ms": 20000,
                "maximum_evidence_events": 64,
                "maximum_scheduler_events": 256,
                "maximum_runtime_ticks": 512
            }),
        );
    }

    let rendered = format!("{}\n", serde_json::to_string_pretty(&plan)?);

    if check {
        if !output_path.exists() || fs::read_to_string(&output_path)? != rendered {
            return Err("tour/public/browser-plan.json is stale; run tour/build-wasm.sh".into());
        }
        println!("tour/public/browser-plan.json is up to date.");
    } else {
        fs::write(&output_path, rendered)?;
        println!("Generated {}", output_path.display());
    }

    Ok(())
}
