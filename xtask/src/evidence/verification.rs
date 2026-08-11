use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    EvidenceKind, EvidenceProvenance, EVIDENCE_SCHEMA, MAX_EVIDENCE_BYTES, MAX_EVIDENCE_OUTPUTS,
};

const MANIFEST_FILE: &str = "manifest.json";
const REQUIRED_BROWSER_OUTPUTS: &[&str] = &[
    "patchbay.capture-declarations",
    "patchbay.overview",
    "patchbay.selected-gear",
    "patchbay.interaction",
    "patchbay.high-contrast",
    "patchbay.disconnected",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedEvidenceResult {
    Complete,
    DiagnosticIncomplete,
}

pub struct VerificationRequest {
    pub root: PathBuf,
    pub commit: String,
    pub result: ExpectedEvidenceResult,
    pub proof_id: String,
    pub suite_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    result: ManifestResult,
    git_commit: String,
    proof_id: String,
    suite_id: String,
    limits: ManifestLimits,
    outputs: Vec<ManifestOutput>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ManifestResult {
    Complete,
    DiagnosticIncomplete,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestLimits {
    maximum_outputs: usize,
    maximum_bytes_per_output: u64,
}

#[derive(Deserialize)]
struct ManifestOutput {
    id: String,
    kind: EvidenceKind,
    path: PathBuf,
    media_type: String,
    required: bool,
    bytes: u64,
    sha256: String,
    #[serde(flatten)]
    provenance: EvidenceProvenance,
}

pub fn verify(request: &VerificationRequest) -> Result<(), String> {
    validate_commit(&request.commit)?;
    let root = request.root.canonicalize().map_err(|error| {
        format!(
            "cannot resolve evidence root {}: {error}",
            request.root.display()
        )
    })?;
    if !root.is_dir() {
        return Err("evidence root is not a directory".into());
    }

    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_metadata = regular_file_metadata(&manifest_path)?;
    if manifest_metadata.len() > MAX_EVIDENCE_BYTES {
        return Err("evidence manifest exceeds the finite size bound".into());
    }
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .map_err(|error| format!("cannot read evidence manifest: {error}"))?,
    )
    .map_err(|error| format!("invalid evidence manifest: {error}"))?;

    let expected_result = match request.result {
        ExpectedEvidenceResult::Complete => ManifestResult::Complete,
        ExpectedEvidenceResult::DiagnosticIncomplete => ManifestResult::DiagnosticIncomplete,
    };
    if manifest.schema != EVIDENCE_SCHEMA
        || manifest.result != expected_result
        || manifest.git_commit != request.commit.to_ascii_lowercase()
        || manifest.proof_id != request.proof_id
        || manifest.suite_id != request.suite_id
    {
        return Err("evidence manifest identity, disposition, or commit does not match".into());
    }
    if manifest.limits.maximum_outputs != MAX_EVIDENCE_OUTPUTS
        || manifest.limits.maximum_bytes_per_output != MAX_EVIDENCE_BYTES
        || manifest.outputs.len() > MAX_EVIDENCE_OUTPUTS
    {
        return Err("evidence manifest does not preserve the reviewed finite bounds".into());
    }

    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::from([PathBuf::from(MANIFEST_FILE)]);
    for output in &manifest.outputs {
        validate_relative_path(&output.path)?;
        if !ids.insert(output.id.as_str()) || !paths.insert(output.path.clone()) {
            return Err("evidence manifest contains a duplicate identity or path".into());
        }
        let candidate = root.join(&output.path);
        let metadata = regular_file_metadata(&candidate)?;
        let resolved = candidate
            .canonicalize()
            .map_err(|error| format!("cannot resolve {}: {error}", candidate.display()))?;
        if !resolved.starts_with(&root) {
            return Err(format!(
                "evidence path '{}' escapes its root",
                output.path.display()
            ));
        }
        if metadata.len() != output.bytes || metadata.len() > MAX_EVIDENCE_BYTES {
            return Err(format!("evidence size does not match for '{}'", output.id));
        }
        if sha256_file(&candidate)? != output.sha256 {
            return Err(format!(
                "evidence digest does not match for '{}'",
                output.id
            ));
        }
    }

    if request.result == ExpectedEvidenceResult::Complete && request.proof_id == "browser-host" {
        for required in REQUIRED_BROWSER_OUTPUTS {
            let output = manifest
                .outputs
                .iter()
                .find(|output| output.id == *required)
                .ok_or_else(|| format!("complete evidence is missing '{required}'"))?;
            if !output.required {
                return Err(format!(
                    "complete evidence does not mark '{required}' required"
                ));
            }
        }
        let screenshots: Vec<_> = manifest
            .outputs
            .iter()
            .filter(|output| output.kind == EvidenceKind::Screenshot)
            .collect();
        if screenshots.len() != REQUIRED_BROWSER_OUTPUTS.len() - 1 {
            return Err("complete browser evidence must contain exactly five screenshots".into());
        }
        for screenshot in screenshots {
            if screenshot.media_type != "image/png"
                || !complete_screenshot_provenance(&screenshot.provenance)
            {
                return Err(format!(
                    "canonical screenshot '{}' lacks exact semantic or camera provenance",
                    screenshot.id
                ));
            }
        }
    }

    reject_undeclared_files(&root, &root, &paths)?;
    println!(
        "verified {:?} evidence for {} at {}",
        request.result,
        request.commit.to_ascii_lowercase(),
        root.display()
    );
    Ok(())
}

fn complete_screenshot_provenance(provenance: &EvidenceProvenance) -> bool {
    !provenance.scenario_id.trim().is_empty()
        && [
            provenance.step_id.as_deref(),
            provenance.browser_engine.as_deref(),
            provenance.browser_version.as_deref(),
            provenance.viewport.as_deref(),
            provenance.device_scale_factor.as_deref(),
            provenance.locale.as_deref(),
            provenance.timezone.as_deref(),
            provenance.presentation_id.as_deref(),
            provenance.presentation_revision.as_deref(),
            provenance.plan_id.as_deref(),
            provenance.active_play_id.as_deref(),
            provenance.manifestation_id.as_deref(),
            provenance.renderer_id.as_deref(),
            provenance.asserted_semantic_disposition.as_deref(),
        ]
        .into_iter()
        .all(|value| value.is_some_and(|value| !value.trim().is_empty()))
}

fn reject_undeclared_files(
    root: &Path,
    directory: &Path,
    declared: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot inspect evidence directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("cannot inspect evidence entry: {error}"))?;
        let metadata = entry
            .file_type()
            .map_err(|error| format!("cannot inspect evidence entry type: {error}"))?;
        let path = entry.path();
        if metadata.is_symlink() {
            return Err(format!("evidence root contains symlink {}", path.display()));
        }
        if metadata.is_dir() {
            reject_undeclared_files(root, &path, declared)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "evidence entry escaped its root")?;
            if !metadata.is_file() || !declared.contains(relative) {
                return Err(format!("undeclared evidence entry {}", relative.display()));
            }
        }
    }
    Ok(())
}

fn regular_file_metadata(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    Ok(metadata)
}

fn validate_commit(value: &str) -> Result<(), String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("expected commit must be an exact 40-character SHA".into())
    }
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "evidence path '{}' is not root-confined",
            path.display()
        ));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open evidence output {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read evidence output {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
