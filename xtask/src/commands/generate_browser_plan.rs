use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn workspace_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn read_contract(workspace_root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let contract_path = workspace_root.join("tour/browser-plan-contract.json");
    let contract: Value = serde_json::from_str(&fs::read_to_string(&contract_path)?)?;
    let contract_object = contract
        .as_object()
        .ok_or("tour/browser-plan-contract.json must contain one JSON object")?;
    if contract_object.get("schema") != Some(&json!("conduit.tour-browser-plan")) {
        return Err("Tour browser plan contract has the wrong schema".into());
    }
    for field in [
        "bounds",
        "evidence_provider",
        "implementation_id",
        "observation_id",
        "placement",
        "semantic_contract",
    ] {
        if !contract_object.contains_key(field) {
            return Err(format!("Tour browser plan contract is missing {field}").into());
        }
    }
    Ok(contract)
}

pub fn check_contract(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    read_contract(workspace_root)?;
    println!("tour/browser-plan-contract.json is valid.");
    Ok(())
}

pub fn run(
    workspace_root: &Path,
    check: bool,
    artifact_dir: &Path,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = workspace_path(workspace_root, artifact_dir);
    let output_path = workspace_path(workspace_root, output);

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
            artifact_dir.join("conduit_web.js"),
            "./conduit_web.js",
        ),
        (
            "conduit-web-wasm",
            artifact_dir.join("conduit_web_bg.wasm"),
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

    let mut plan = read_contract(workspace_root)?;
    plan.as_object_mut()
        .expect("validated browser plan contract")
        .insert("artifacts".to_string(), json!(artifacts));
    let identity_bytes = serde_json::to_vec(&plan)?;
    let plan_identity = format!("{:x}", Sha256::digest(&identity_bytes));
    plan.as_object_mut()
        .expect("validated browser plan contract")
        .insert("plan_identity".to_string(), json!(plan_identity));

    let rendered = format!("{}\n", serde_json::to_string_pretty(&plan)?);

    if check {
        if !output_path.exists() || fs::read_to_string(&output_path)? != rendered {
            return Err(
                format!("{} is stale; run tour/build-wasm.sh", output_path.display()).into(),
            );
        }
        println!("{} is up to date.", output_path.display());
    } else {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, rendered)?;
        println!("Generated {}", output_path.display());
    }

    Ok(())
}
