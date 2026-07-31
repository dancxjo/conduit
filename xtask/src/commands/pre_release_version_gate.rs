use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Inventory {
    schema_version: u32,
    workspace_version: String,
    policy: String,
    classifications: Vec<Classification>,
    released_obligations: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Classification {
    family: String,
    classification: String,
    locations: Vec<String>,
}

#[derive(Deserialize)]
struct ExceptionLedger {
    schema_version: u32,
    releases: Vec<serde_json::Value>,
}

const REQUIRED_FAMILIES: &[&str] = &[
    "workspace-crates",
    "panel-grammar",
    "source-ast",
    "semantic-source-hash",
    "lowered-source",
    "canonical-form",
    "execution-plan",
    "run-stream",
    "result-diagnostic-evidence-inspection",
    "package-manifest-catalog",
    "tour-patchbay-browser-plan",
    "distributed-binding-envelope",
    "semantic-type-and-port-compatibility",
    "live-plan-and-checkpoint-transition",
];

const DISPLACED_CODE_MARKERS: &[&str] = &[
    "migrate_directional_syntax_v3",
    "migrate_lowered_source_v1",
    "SOURCE_AST_SCHEMA_V1",
    "SOURCE_AST_SCHEMA_V2",
    "SOURCE_AST_SCHEMA_V3",
    "LOWERED_SOURCE_SCHEMA_V1",
    "EXECUTION_PLAN_SCHEMA_V1",
    "EXECUTION_PLAN_SCHEMA_V2",
    "RunStreamVersion",
    "WithdrawnV1",
];

pub fn run(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let inventory_path = workspace_root.join("inventory/pre-release-versions.json");
    let inventory: Inventory = serde_json::from_slice(&fs::read(&inventory_path)?)?;
    if inventory.schema_version != 0
        || inventory.workspace_version != "0.0.0-dev"
        || inventory.policy != "one-current-draft"
        || !inventory.released_obligations.is_empty()
    {
        return Err(
            "pre-release version inventory does not describe the one-current-draft policy".into(),
        );
    }
    for required in REQUIRED_FAMILIES {
        let Some(entry) = inventory
            .classifications
            .iter()
            .find(|entry| entry.family == *required)
        else {
            return Err(
                format!("pre-release version inventory is missing family {required}").into(),
            );
        };
        if entry.classification.is_empty() || entry.locations.is_empty() {
            return Err(format!("pre-release inventory family {required} is unclassified").into());
        }
    }

    let ledger: ExceptionLedger = serde_json::from_slice(&fs::read(
        workspace_root.join("inventory/release-compatibility-exceptions.json"),
    )?)?;
    if ledger.schema_version != 0 || !ledger.releases.is_empty() {
        return Err("pre-release compatibility exception ledger must be empty".into());
    }

    let manifest = fs::read_to_string(workspace_root.join("Cargo.toml"))?;
    if !manifest.contains("version = \"0.0.0-dev\"") {
        return Err("workspace package version must be 0.0.0-dev".into());
    }

    let mut files = Vec::new();
    collect_files(workspace_root, workspace_root, &mut files)?;
    for path in files {
        let relative = path.strip_prefix(workspace_root).unwrap_or(&path);
        let relative_text = relative.to_string_lossy();
        if is_candidate_path(relative)
            && relative
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(has_release_suffix)
        {
            return Err(format!(
                "candidate artifact has a release-looking draft suffix: {relative_text}"
            )
            .into());
        }
        if path.extension().and_then(|value| value.to_str()) == Some("panel") {
            let source = fs::read_to_string(&path)?;
            if source
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .is_some_and(|line| line != "panel 0")
            {
                return Err(
                    format!("canonical Panel source is not panel 0: {relative_text}").into(),
                );
            }
        }
        if is_production_rust(relative) {
            let source = fs::read_to_string(&path)?;
            for marker in DISPLACED_CODE_MARKERS {
                if source.contains(marker) {
                    return Err(format!(
                        "displaced unreleased compatibility marker {marker:?} remains in {relative_text}"
                    )
                    .into());
                }
            }
        }
    }

    println!(
        "pre-release version gate passed: one current draft, topical candidates, empty release ledger"
    );
    Ok(())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if path.is_dir() {
            if matches!(
                relative
                    .components()
                    .next()
                    .and_then(|part| part.as_os_str().to_str()),
                Some(".git" | "target" | "node_modules")
            ) {
                continue;
            }
            collect_files(root, &path, output)?;
        } else {
            output.push(path);
        }
    }
    Ok(())
}

fn is_candidate_path(path: &Path) -> bool {
    matches!(
        path.components()
            .next()
            .and_then(|part| part.as_os_str().to_str()),
        Some("spec" | "conformance")
    )
}

fn has_release_suffix(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.windows(2).enumerate().any(|(index, pair)| {
        pair[0] == b'-'
            && pair[1] == b'v'
            && bytes
                .get(index + 2)
                .is_some_and(|byte| byte.is_ascii_digit() && *byte != b'0')
    })
}

fn is_production_rust(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("rs")
        && !path.components().any(|part| part.as_os_str() == "tests")
        && path.file_name().and_then(|value| value.to_str()) != Some("pre_release_version_gate.rs")
}
