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

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const EVIDENCE_SCHEMA: &str = "conduit.evidence-manifest/v1";
pub const MAX_EVIDENCE_OUTPUTS: usize = 64;
pub const MAX_EVIDENCE_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)] // Both kinds are contract surface; #822 declares the first captures.
pub enum EvidenceKind {
    Screenshot,
    MachineReadableManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceResult {
    Complete,
    DiagnosticIncomplete,
}

#[derive(Debug, Clone)]
pub struct EvidenceOutput {
    pub id: String,
    pub kind: EvidenceKind,
    pub path: PathBuf,
    pub media_type: String,
    pub required: bool,
    pub provenance: EvidenceProvenance,
}

#[derive(Debug, Clone, Default, Serialize)]
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
            .then(|| std::env::var("GITHUB_SHA").ok())
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

    #[allow(dead_code)]
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
        return validate_git_commit(sha, "GITHUB_SHA");
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
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("conduit-evidence-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn manifest(root: &Path) -> EvidenceManifest {
        EvidenceManifest::new(
            root,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "proof",
            "suite",
        )
        .unwrap()
    }

    fn output(id: &str, path: &str, required: bool) -> EvidenceOutput {
        EvidenceOutput {
            id: id.into(),
            kind: EvidenceKind::Screenshot,
            path: path.into(),
            media_type: "image/png".into(),
            required,
            provenance: EvidenceProvenance {
                scenario_id: "scenario".into(),
                asserted_semantic_disposition: Some("delivered".into()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn complete_manifest_digest_binds_exact_bytes() {
        let root = temporary_root("digest");
        fs::write(root.join("capture.png"), b"canonical bytes").unwrap();
        let mut evidence = manifest(&root);
        evidence
            .declare(output("capture", "capture.png", true))
            .unwrap();
        evidence.finish(EvidenceResult::Complete).unwrap();

        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(document["schema"], EVIDENCE_SCHEMA);
        assert_eq!(document["result"], "complete");
        assert_eq!(document["outputs"][0]["bytes"], 15);
        assert_eq!(
            document["outputs"][0]["sha256"],
            "a62cbfa5ab07ca2085092bb00488c2256b93dedcd2a8bd88e65b6ee055d7a499"
        );
        assert_eq!(document["outputs"][0]["scenario_id"], "scenario");
        assert_eq!(
            document["outputs"][0]["asserted_semantic_disposition"],
            "delivered"
        );
        assert!(document.get("timestamp").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_required_output_is_manifested_as_incomplete_and_refused() {
        let root = temporary_root("missing");
        let mut evidence = manifest(&root);
        evidence
            .declare(output("required", "missing.png", true))
            .unwrap();
        let error = evidence.finish(EvidenceResult::Complete).unwrap_err();
        assert!(error.contains("required"));
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(document["result"], "diagnostic-incomplete");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_ids_and_escaping_paths_are_rejected() {
        let root = temporary_root("bounds");
        let mut evidence = manifest(&root);
        evidence.declare(output("one", "one.png", false)).unwrap();
        assert!(evidence.declare(output("one", "two.png", false)).is_err());
        assert!(evidence
            .declare(output("escape", "../escape.png", false))
            .is_err());
        assert!(evidence
            .declare(output("alias", "./one.png", false))
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_paths_and_count_overflow_are_rejected() {
        let root = temporary_root("count");
        let mut evidence = manifest(&root);
        evidence.declare(output("one", "same.png", false)).unwrap();
        evidence.declare(output("two", "same.png", false)).unwrap();
        assert!(evidence.finish(EvidenceResult::Complete).is_err());

        let mut evidence = manifest(&root);
        for index in 0..MAX_EVIDENCE_OUTPUTS {
            evidence
                .declare(output(
                    &format!("output-{index}"),
                    &format!("output-{index}.png"),
                    false,
                ))
                .unwrap();
        }
        assert!(evidence
            .declare(output("overflow", "overflow.png", false))
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diagnostic_artifacts_cannot_look_complete() {
        let root = temporary_root("diagnostic");
        fs::write(root.join("capture.png"), b"diagnostic").unwrap();
        let mut evidence = manifest(&root);
        evidence
            .declare(output("capture", "capture.png", false))
            .unwrap();
        evidence
            .finish(EvidenceResult::DiagnosticIncomplete)
            .unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(document["result"], "diagnostic-incomplete");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn actions_checkout_sha_is_validated_without_invoking_git() {
        let sha = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";
        assert_eq!(
            exact_git_commit(Path::new("/not/a/checkout"), Some(sha)).unwrap(),
            sha.to_ascii_lowercase()
        );
        assert!(exact_git_commit(Path::new("/not/a/checkout"), Some("floating-main")).is_err());
    }
}
