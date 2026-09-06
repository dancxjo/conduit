//! Bounded, proof-native declarations and provenance for human-inspectable evidence.
//!
//! Semantic proof remains authoritative. Evidence is emitted only as a digest-bound
//! presentation of that proof and is explicitly complete or diagnostic/incomplete.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod documentation;
mod gallery;
mod verification;

pub use documentation::{verify_documentation_references, DocumentationReferenceRequest};
pub use gallery::{publish_gallery, GalleryRequest};
pub use verification::{
    verify, ExpectedEvidenceResult, VerificationRequest, VerifiedEvidence, VerifiedOutput,
};

pub const EVIDENCE_SCHEMA: &str = "conduit.evidence-manifest/v1";
pub const MAX_EVIDENCE_OUTPUTS: usize = 64;
pub const MAX_EVIDENCE_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_FILE: &str = "manifest.json";
const CAPTURE_DECLARATION_SCHEMA: &str = "conduit.capture-declarations/v1";

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    Screenshot,
    MachineReadableManifest,
    ConsoleTranscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceResult {
    Complete,
    DiagnosticIncomplete,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceOutput {
    pub id: String,
    pub kind: EvidenceKind,
    pub path: PathBuf,
    pub media_type: String,
    pub required: bool,
    pub provenance: EvidenceProvenance,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceProvenance {
    pub scenario_id: String,
    pub step_id: Option<String>,
    pub browser_engine: Option<String>,
    pub browser_version: Option<String>,
    pub viewport: Option<String>,
    pub device_scale_factor: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub presentation_id: Option<String>,
    pub presentation_revision: Option<String>,
    pub plan_id: Option<String>,
    pub active_play_id: Option<String>,
    pub manifestation_id: Option<String>,
    pub renderer_id: Option<String>,
    pub asserted_semantic_disposition: Option<String>,
    pub proof_class: Option<String>,
    pub architecture: Option<String>,
    pub architecture_rung: Option<String>,
    pub emulator: Option<String>,
    pub emulator_version: Option<String>,
    pub machine: Option<String>,
    pub firmware: Option<String>,
    pub host_id: Option<String>,
    pub boot_id: Option<String>,
    pub kernel_artifact_id: Option<String>,
    pub kernel_artifact_sha256: Option<String>,
    pub capture_trigger: Option<String>,
    pub capture_byte_limit: Option<u64>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub physical_evidence: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Manifest {
    schema: &'static str,
    result: EvidenceResult,
    git_commit: String,
    proof_id: String,
    suite_id: String,
    limits: ManifestLimits,
    outputs: Vec<ManifestOutput>,
}

#[derive(Debug, Serialize)]
struct ManifestLimits {
    maximum_outputs: usize,
    maximum_bytes_per_output: u64,
}

#[derive(Debug, Serialize)]
struct ManifestOutput {
    id: String,
    kind: EvidenceKind,
    path: String,
    media_type: String,
    required: bool,
    bytes: u64,
    sha256: String,
    #[serde(flatten)]
    provenance: EvidenceProvenance,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureDeclarations {
    schema: String,
    outputs: Vec<EvidenceOutput>,
}

pub struct EvidenceManifest {
    root: PathBuf,
    git_commit: String,
    proof_id: String,
    suite_id: String,
    declarations: Vec<EvidenceOutput>,
}

impl EvidenceManifest {
    pub fn new(
        root: &Path,
        workspace_root: &Path,
        proof_id: impl Into<String>,
        suite_id: impl Into<String>,
    ) -> Result<Self, String> {
        let proof_id = proof_id.into();
        let suite_id = suite_id.into();
        if proof_id.trim().is_empty() || suite_id.trim().is_empty() {
            return Err("evidence proof and suite identities must be non-empty".into());
        }
        fs::create_dir_all(root)
            .map_err(|error| format!("cannot create evidence root {}: {error}", root.display()))?;
        let root = root
            .canonicalize()
            .map_err(|error| format!("cannot resolve evidence root {}: {error}", root.display()))?;
        let actions_sha = (std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true"))
            .then(|| {
                std::env::var("CONDUIT_CHECKOUT_SHA")
                    .or_else(|_| std::env::var("GITHUB_SHA"))
                    .ok()
            })
            .flatten();
        let git_commit = exact_git_commit(workspace_root, actions_sha.as_deref())?;

        Ok(Self {
            root,
            git_commit,
            proof_id,
            suite_id,
            declarations: Vec::new(),
        })
    }

    pub fn declare(&mut self, output: EvidenceOutput) -> Result<(), String> {
        if self.declarations.len() == MAX_EVIDENCE_OUTPUTS {
            return Err(format!(
                "evidence output count exceeds bounded maximum {MAX_EVIDENCE_OUTPUTS}"
            ));
        }
        if output.id.trim().is_empty()
            || output.media_type.trim().is_empty()
            || output.provenance.scenario_id.trim().is_empty()
        {
            return Err(
                "evidence identity, media type, and scenario identity must be non-empty".into(),
            );
        }
        validate_relative_path(&output.path)?;
        if self
            .declarations
            .iter()
            .any(|existing| existing.id == output.id)
        {
            return Err(format!("duplicate evidence identity '{}'", output.id));
        }
        self.declarations.push(output);
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn import_capture_declarations(
        &mut self,
        path: &Path,
        required_ids: &[&str],
    ) -> Result<(), String> {
        validate_relative_path(path)?;
        let candidate = self.root.join(path);
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            format!(
                "cannot inspect capture declarations {}: {error}",
                candidate.display()
            )
        })?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_EVIDENCE_BYTES {
            return Err("capture declarations must be one bounded regular file".into());
        }
        let resolved = candidate.canonicalize().map_err(|error| {
            format!(
                "cannot resolve capture declarations {}: {error}",
                candidate.display()
            )
        })?;
        if !resolved.starts_with(&self.root) {
            return Err("capture declarations escape the configured evidence root".into());
        }
        let bytes = fs::read(&candidate).map_err(|error| {
            format!(
                "cannot read capture declarations {}: {error}",
                candidate.display()
            )
        })?;
        let declarations: CaptureDeclarations = serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid capture declarations: {error}"))?;
        if declarations.schema != CAPTURE_DECLARATION_SCHEMA {
            return Err(format!(
                "unsupported capture declaration schema '{}'",
                declarations.schema
            ));
        }
        if declarations.outputs.len() > MAX_EVIDENCE_OUTPUTS {
            return Err("capture declaration count exceeds the evidence bound".into());
        }
        for output in declarations.outputs {
            self.declare(output)?;
        }
        for required in required_ids {
            if !self
                .declarations
                .iter()
                .any(|output| output.id == *required)
            {
                return Err(format!(
                    "required capture declaration '{required}' is missing"
                ));
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, requested_result: EvidenceResult) -> Result<(), String> {
        let mut seen_paths = BTreeSet::new();
        let mut outputs = Vec::with_capacity(self.declarations.len());
        let mut missing_required = Vec::new();

        for declaration in &self.declarations {
            validate_relative_path(&declaration.path)?;
            if !seen_paths.insert(declaration.path.clone()) {
                return Err(format!(
                    "duplicate evidence path '{}'",
                    declaration.path.display()
                ));
            }
            let candidate = self.root.join(&declaration.path);
            if !candidate.exists() {
                if declaration.required {
                    missing_required.push(declaration.id.clone());
                }
                continue;
            }
            let resolved = candidate.canonicalize().map_err(|error| {
                format!(
                    "cannot resolve evidence output {}: {error}",
                    candidate.display()
                )
            })?;
            if !resolved.starts_with(&self.root) {
                return Err(format!(
                    "evidence output '{}' escapes the configured root",
                    declaration.id
                ));
            }
            let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
                format!(
                    "cannot inspect evidence output {}: {error}",
                    candidate.display()
                )
            })?;
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "evidence output '{}' is not a regular file",
                    declaration.id
                ));
            }
            if metadata.len() > MAX_EVIDENCE_BYTES {
                return Err(format!(
                    "evidence output '{}' exceeds {} bytes",
                    declaration.id, MAX_EVIDENCE_BYTES
                ));
            }
            outputs.push(ManifestOutput {
                id: declaration.id.clone(),
                kind: declaration.kind,
                path: path_for_manifest(&declaration.path)?,
                media_type: declaration.media_type.clone(),
                required: declaration.required,
                bytes: metadata.len(),
                sha256: sha256_file(&candidate)?,
                provenance: declaration.provenance.clone(),
            });
        }

        let result = if requested_result == EvidenceResult::Complete && missing_required.is_empty()
        {
            EvidenceResult::Complete
        } else {
            EvidenceResult::DiagnosticIncomplete
        };
        let manifest = Manifest {
            schema: EVIDENCE_SCHEMA,
            result,
            git_commit: self.git_commit.clone(),
            proof_id: self.proof_id.clone(),
            suite_id: self.suite_id.clone(),
            limits: ManifestLimits {
                maximum_outputs: MAX_EVIDENCE_OUTPUTS,
                maximum_bytes_per_output: MAX_EVIDENCE_BYTES,
            },
            outputs,
        };
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("cannot serialize evidence manifest: {error}"))?;
        fs::write(self.root.join(MANIFEST_FILE), bytes)
            .map_err(|error| format!("cannot write evidence manifest: {error}"))?;

        if missing_required.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "required evidence outputs are missing: {}",
                missing_required.join(", ")
            ))
        }
    }
}

fn exact_git_commit(workspace_root: &Path, actions_sha: Option<&str>) -> Result<String, String> {
    if let Some(sha) = actions_sha {
        return validate_git_commit(sha, "Actions checkout identity");
    }

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("cannot determine evidence git commit: {error}"))?;
    if !output.status.success() {
        return Err("git rev-parse HEAD failed for evidence manifest".into());
    }
    validate_git_commit(
        String::from_utf8_lossy(&output.stdout).trim(),
        "git rev-parse HEAD",
    )
}

fn validate_git_commit(value: &str, source: &str) -> Result<String, String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(format!(
            "{source} did not provide an exact 40-character commit SHA"
        ))
    }
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("evidence output path must be a non-empty relative path".into());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "evidence output path '{}' is not root-confined",
            path.display()
        ));
    }
    Ok(())
}

fn path_for_manifest(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "evidence output path is not valid UTF-8".into())
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

#[cfg(test)]
mod tests;
