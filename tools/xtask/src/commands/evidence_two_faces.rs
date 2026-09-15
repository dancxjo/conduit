//! One deterministic Presentation retained through native and browser renderers.

use crate::evidence::{
    EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
};
use crate::workspace::workspace_root;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn run(output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_root()?;
    let output = if output.is_absolute() {
        output
    } else {
        workspace.join(output)
    };
    let parent = output.parent().ok_or("two-faces output has no parent")?;
    std::fs::create_dir_all(parent)?;
    std::fs::create_dir(&output)
        .map_err(|error| format!("create new two-faces evidence directory: {error}"))?;
    let mut guard = NewDirectory::new(output.clone());

    command(
        Command::new(cargo()).current_dir(&workspace).args([
            "build",
            "--locked",
            "-p",
            "patchbay-native",
            "-p",
            "patchbay-html",
        ]),
        "build the two renderer entrances",
    )?;
    command(
        Command::new(workspace.join("target/debug/patchbay-native"))
            .current_dir(&workspace)
            .arg("--two-faces-capture")
            .arg(&output),
        "capture native renderer pixels",
    )?;
    command(
        Command::new("node")
            .current_dir(&workspace)
            .env("CONDUIT_TWO_FACES_EVIDENCE_ROOT", &output)
            .args([
                "proof/browser/node_modules/@playwright/test/cli.js",
                "test",
                "--config",
                "proof/browser/one-form-two-faces.playwright.config.mjs",
                "--project",
                "chromium",
                "--workers",
                "1",
                "--retries",
                "0",
            ]),
        "capture pinned-browser renderer pixels",
    )?;

    finish_manifest(&output, &workspace)?;
    guard.retain();
    println!("ONE FORM, TWO FACES COMPLETE: {}", output.display());
    Ok(())
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn command(command: &mut Command, description: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{description}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{description} failed with {status}"))
    }
}

fn finish_manifest(root: &Path, workspace: &Path) -> Result<(), String> {
    let native = read_receipt(&root.join("native.json"))?;
    let browser = read_receipt(&root.join("browser.json"))?;
    let presentation_id = text(&native, "presentation_id")?;
    let browser_presentation_id = text(&browser, "presentation_id")?;
    let revision = integer(&native, "presentation_revision")?;
    if presentation_id != browser_presentation_id
        || revision != integer(&browser, "presentation_revision")?
        || native.get("presentation_basis") != browser.get("presentation_basis")
    {
        return Err("native and browser receipts do not identify one exact Presentation".into());
    }
    let mut manifest = EvidenceManifest::new(
        root,
        workspace,
        "journey-one-form-two-faces",
        "journey-gallery",
    )?;
    for (id, kind, path, receipt, proof_class) in [
        (
            "two-faces.native-frame",
            EvidenceKind::Screenshot,
            "native.png",
            &native,
            "native-software-renderer",
        ),
        (
            "two-faces.native-receipt",
            EvidenceKind::MachineReadableManifest,
            "native.json",
            &native,
            "native-software-renderer",
        ),
        (
            "two-faces.browser-frame",
            EvidenceKind::Screenshot,
            "browser.png",
            &browser,
            "live-browser",
        ),
        (
            "two-faces.browser-receipt",
            EvidenceKind::MachineReadableManifest,
            "browser.json",
            &browser,
            "live-browser",
        ),
    ] {
        manifest.declare(EvidenceOutput {
            id: id.into(),
            kind,
            path: path.into(),
            media_type: if path.ends_with(".png") {
                "image/png".into()
            } else {
                "application/json".into()
            },
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "one-form-two-faces.front-door@1".into(),
                presentation_id: Some(presentation_id.into()),
                presentation_revision: Some(revision.to_string()),
                manifestation_id: Some(text(receipt, "manifestation_id")?.into()),
                plan_id: Some(text(receipt, "renderer_plan_id")?.into()),
                active_play_id: Some(text(receipt, "renderer_play_id")?.into()),
                renderer_id: Some(text(receipt, "renderer_implementation")?.into()),
                browser_engine: receipt
                    .get("browser_engine")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                browser_version: receipt
                    .get("browser_version")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                viewport: receipt
                    .get("viewport")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                device_scale_factor: receipt
                    .get("device_scale_factor")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                locale: receipt
                    .get("locale")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                timezone: receipt
                    .get("timezone")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                asserted_semantic_disposition: Some("manifestation-available".into()),
                proof_class: Some(proof_class.into()),
                ..EvidenceProvenance::default()
            },
        })?;
    }
    manifest.finish(EvidenceResult::Complete)
}

fn read_receipt(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {}: {error}", path.display()))
}

fn text<'a>(receipt: &'a Value, field: &str) -> Result<&'a str, String> {
    receipt
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("two-faces receipt lacks {field}"))
}

fn integer(receipt: &Value, field: &str) -> Result<u64, String> {
    receipt
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("two-faces receipt lacks {field}"))
}

struct NewDirectory {
    path: PathBuf,
    retain: bool,
}

impl NewDirectory {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            retain: false,
        }
    }

    fn retain(&mut self) {
        self.retain = true;
    }
}

impl Drop for NewDirectory {
    fn drop(&mut self) {
        if !self.retain {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
