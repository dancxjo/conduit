use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    EvidenceKind, EvidenceProvenance, EVIDENCE_SCHEMA, MAX_EVIDENCE_BYTES, MAX_EVIDENCE_OUTPUTS,
};

const MANIFEST_FILE: &str = "manifest.json";
const REQUIRED_BROWSER_OUTPUTS: &[&str] = &[
    "patchbay.capture-declarations",
    "patchbay.overview",
    "patchbay.selected-gear",
    "patchbay.plan-lens",
    "patchbay.play-lens",
    "patchbay.signs-lens",
    "patchbay.route-recovery",
    "patchbay.interaction",
    "patchbay.high-contrast",
    "patchbay.disconnected",
    "patchbay.responsive",
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

#[derive(Debug)]
pub struct VerifiedEvidence {
    pub commit: String,
    pub proof_id: String,
    pub suite_id: String,
    pub outputs: Vec<VerifiedOutput>,
}

#[derive(Debug)]
pub struct VerifiedOutput {
    pub id: String,
    pub kind: EvidenceKind,
    pub path: PathBuf,
    pub media_type: String,
    pub bytes: u64,
    pub sha256: String,
    pub provenance: EvidenceProvenance,
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

pub fn verify(request: &VerificationRequest) -> Result<VerifiedEvidence, String> {
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
            return Err("complete browser evidence must contain exactly ten screenshots".into());
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
    if request.result == ExpectedEvidenceResult::Complete && request.proof_id == "conduitos-x86_64"
    {
        verify_conduitos_console(&root, &manifest, &request.commit)?;
    }
    if request.result == ExpectedEvidenceResult::Complete
        && request.proof_id == "journey-hears-speaks"
    {
        verify_hears_speaks(&root, &manifest)?;
    }
    if request.result == ExpectedEvidenceResult::Complete
        && request.proof_id == "journey-one-form-two-faces"
    {
        verify_one_form_two_faces(&root, &manifest)?;
    }
    if request.result == ExpectedEvidenceResult::Complete
        && request.proof_id == "journey-little-life"
    {
        verify_little_life(&root, &manifest)?;
    }

    reject_undeclared_files(&root, &root, &paths)?;
    println!(
        "verified {:?} evidence for {} at {}",
        request.result,
        request.commit.to_ascii_lowercase(),
        root.display()
    );
    Ok(VerifiedEvidence {
        commit: manifest.git_commit,
        proof_id: manifest.proof_id,
        suite_id: manifest.suite_id,
        outputs: manifest
            .outputs
            .into_iter()
            .map(|output| VerifiedOutput {
                id: output.id,
                kind: output.kind,
                path: output.path,
                media_type: output.media_type,
                bytes: output.bytes,
                sha256: output.sha256,
                provenance: output.provenance,
            })
            .collect(),
    })
}

fn verify_one_form_two_faces(root: &Path, manifest: &Manifest) -> Result<(), String> {
    let expected = [
        (
            "two-faces.native-frame",
            EvidenceKind::Screenshot,
            "native.png",
        ),
        (
            "two-faces.native-receipt",
            EvidenceKind::MachineReadableManifest,
            "native.json",
        ),
        (
            "two-faces.browser-frame",
            EvidenceKind::Screenshot,
            "browser.png",
        ),
        (
            "two-faces.browser-receipt",
            EvidenceKind::MachineReadableManifest,
            "browser.json",
        ),
    ];
    if manifest.outputs.len() != expected.len() {
        return Err("complete two-faces evidence must contain exactly four outputs".into());
    }
    let mut presentation = None;
    for (id, kind, path) in expected {
        let output = manifest
            .outputs
            .iter()
            .find(|output| output.id == id)
            .ok_or_else(|| format!("complete two-faces evidence is missing '{id}'"))?;
        let native = id.contains("native");
        if !output.required
            || output.kind != kind
            || output.path != Path::new(path)
            || output.media_type
                != if kind == EvidenceKind::Screenshot {
                    "image/png"
                } else {
                    "application/json"
                }
            || output.provenance.scenario_id != "one-form-two-faces.front-door@1"
            || output.provenance.proof_class.as_deref()
                != Some(if native {
                    "native-software-renderer"
                } else {
                    "live-browser"
                })
            || output.provenance.renderer_id.as_deref()
                != Some(if native {
                    "presentation/renderer-wayland@1"
                } else {
                    "presentation/renderer-dom-svg@1"
                })
            || output.provenance.asserted_semantic_disposition.as_deref()
                != Some("manifestation-available")
            || [
                output.provenance.presentation_id.as_deref(),
                output.provenance.presentation_revision.as_deref(),
                output.provenance.plan_id.as_deref(),
                output.provenance.active_play_id.as_deref(),
                output.provenance.manifestation_id.as_deref(),
            ]
            .into_iter()
            .any(|value| value.is_none_or(str::is_empty))
            || (!native
                && [
                    output.provenance.browser_engine.as_deref(),
                    output.provenance.browser_version.as_deref(),
                    output.provenance.viewport.as_deref(),
                    output.provenance.device_scale_factor.as_deref(),
                    output.provenance.locale.as_deref(),
                    output.provenance.timezone.as_deref(),
                ]
                .into_iter()
                .any(|value| value.is_none_or(str::is_empty)))
        {
            return Err(format!(
                "two-faces output '{id}' lacks exact typed provenance"
            ));
        }
        let identity = (
            output.provenance.presentation_id.as_deref().unwrap(),
            output.provenance.presentation_revision.as_deref().unwrap(),
        );
        if presentation.is_some_and(|prior| prior != identity) {
            return Err("two-faces outputs do not share one Presentation identity".into());
        }
        presentation = Some(identity);
        if kind == EvidenceKind::Screenshot {
            let bytes = fs::read(root.join(path))
                .map_err(|error| format!("read two-faces PNG: {error}"))?;
            if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR"
            {
                return Err(format!("two-faces output '{id}' is not a bounded PNG"));
            }
        }
    }
    for native in [true, false] {
        verify_two_faces_receipt(root, manifest, native)?;
    }
    Ok(())
}

fn verify_two_faces_receipt(root: &Path, manifest: &Manifest, native: bool) -> Result<(), String> {
    let side = if native { "native" } else { "browser" };
    let output = manifest
        .outputs
        .iter()
        .find(|output| output.id == format!("two-faces.{side}-receipt"))
        .ok_or_else(|| format!("two-faces evidence lacks {side} receipt"))?;
    let receipt: Value = serde_json::from_slice(
        &fs::read(root.join(&output.path))
            .map_err(|error| format!("read two-faces {side} receipt: {error}"))?,
    )
    .map_err(|error| format!("decode two-faces {side} receipt: {error}"))?;
    let expected = [
        (
            "presentation_id",
            output.provenance.presentation_id.as_deref(),
        ),
        (
            "presentation_revision",
            output.provenance.presentation_revision.as_deref(),
        ),
        ("renderer_plan_id", output.provenance.plan_id.as_deref()),
        (
            "renderer_play_id",
            output.provenance.active_play_id.as_deref(),
        ),
        (
            "manifestation_id",
            output.provenance.manifestation_id.as_deref(),
        ),
        (
            "renderer_implementation",
            output.provenance.renderer_id.as_deref(),
        ),
    ];
    for (field, provenance) in expected {
        let value = receipt
            .get(field)
            .ok_or_else(|| format!("two-faces {side} receipt lacks required field '{field}'"))?;
        let value = value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_u64().map(|value| value.to_string()));
        if value.as_deref() != provenance {
            return Err(format!(
                "two-faces {side} receipt field '{field}' disagrees with manifest provenance"
            ));
        }
    }
    if receipt.get("lifecycle").and_then(Value::as_str) != Some("available")
        || receipt
            .get("pixel_equality_claimed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(format!(
            "two-faces {side} receipt makes an invalid manifestation claim"
        ));
    }
    if !native {
        for (field, provenance) in [
            (
                "browser_engine",
                output.provenance.browser_engine.as_deref(),
            ),
            (
                "browser_version",
                output.provenance.browser_version.as_deref(),
            ),
            ("viewport", output.provenance.viewport.as_deref()),
            (
                "device_scale_factor",
                output.provenance.device_scale_factor.as_deref(),
            ),
            ("locale", output.provenance.locale.as_deref()),
            ("timezone", output.provenance.timezone.as_deref()),
        ] {
            if receipt.get(field).and_then(Value::as_str) != provenance {
                return Err(format!(
                    "two-faces browser receipt field '{field}' disagrees with manifest provenance"
                ));
            }
        }
    }
    Ok(())
}

fn verify_little_life(root: &Path, manifest: &Manifest) -> Result<(), String> {
    let expected = [
        ("little-life.t0", EvidenceKind::Screenshot, "t000.png"),
        ("little-life.t1", EvidenceKind::Screenshot, "t001.png"),
        ("little-life.t8", EvidenceKind::Screenshot, "t008.png"),
        ("little-life.t32", EvidenceKind::Screenshot, "t032.png"),
        (
            "little-life.presentation",
            EvidenceKind::ConsoleTranscript,
            "presentation.txt",
        ),
        (
            "little-life.execution",
            EvidenceKind::MachineReadableManifest,
            "execution.json",
        ),
    ];
    if manifest.outputs.len() != expected.len() {
        return Err("complete Little Life evidence must contain exactly six outputs".into());
    }
    let mut identity = None;
    for (id, kind, path) in expected {
        let output = manifest
            .outputs
            .iter()
            .find(|output| output.id == id)
            .ok_or_else(|| format!("complete Little Life evidence is missing '{id}'"))?;
        let generation = id
            .strip_prefix("little-life.t")
            .and_then(|value| value.parse::<u64>().ok());
        let seed = generation == Some(0);
        if !output.required
            || output.kind != kind
            || output.path != Path::new(path)
            || output.media_type
                != match kind {
                    EvidenceKind::Screenshot => "image/png",
                    EvidenceKind::ConsoleTranscript => "text/plain; charset=utf-8",
                    EvidenceKind::MachineReadableManifest => "application/json",
                    EvidenceKind::Audio => unreachable!(),
                }
            || output.provenance.scenario_id != "little-life.orbium-lenia@1"
            || output.provenance.asserted_semantic_disposition.as_deref() != Some("completed")
            || output.provenance.physical_evidence != Some(false)
            || output
                .provenance
                .plan_id
                .as_deref()
                .is_none_or(str::is_empty)
            || output
                .provenance
                .active_play_id
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(format!("Little Life output '{id}' lacks exact provenance"));
        }
        let current = (
            output.provenance.plan_id.as_deref().unwrap(),
            output.provenance.active_play_id.as_deref().unwrap(),
        );
        if identity.is_some_and(|prior| prior != current) {
            return Err("Little Life outputs do not share one exact Plan and Play".into());
        }
        identity = Some(current);
        if let Some(generation) = generation {
            let expected_renderer = if seed {
                "presentation/gray8-bitmap@1"
            } else {
                "std/kernel-present-scalar-field@1"
            };
            let expected_class = if seed {
                "deterministic-orbium-seed-bitmap"
            } else {
                "std-scalar-field-terminal-presentation"
            };
            let expected_step = format!("generation-{generation}");
            if output.provenance.step_id.as_deref() != Some(expected_step.as_str())
                || output.provenance.renderer_id.as_deref() != Some(expected_renderer)
                || output.provenance.proof_class.as_deref() != Some(expected_class)
            {
                return Err(format!("Little Life checkpoint '{id}' has wrong semantics"));
            }
            verify_png_dimensions(root, output, if seed { (32, 32) } else { (640, 320) })?;
        } else if id == "little-life.presentation"
            && (output.provenance.step_id.as_deref() != Some("generation-32")
                || output.provenance.renderer_id.as_deref()
                    != Some("std/kernel-present-scalar-field@1")
                || output.provenance.proof_class.as_deref()
                    != Some("std-scalar-field-terminal-presentation"))
        {
            return Err("Little Life transcript has wrong proof provenance".into());
        } else if id == "little-life.execution"
            && (output.provenance.step_id.as_deref() != Some("plan-terminal")
                || output.provenance.renderer_id.is_some()
                || output.provenance.proof_class.as_deref()
                    != Some("ordinary-plan-play-execution-report"))
        {
            return Err("Little Life execution report has wrong proof provenance".into());
        }
    }
    verify_little_life_transcript(root)?;
    verify_little_life_execution(root, identity.unwrap())
}

fn verify_png_dimensions(
    root: &Path,
    output: &ManifestOutput,
    expected: (u32, u32),
) -> Result<(), String> {
    let bytes = fs::read(root.join(&output.path))
        .map_err(|error| format!("read Little Life PNG: {error}"))?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err(format!("Little Life output '{}' is not a PNG", output.id));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if (width, height) != expected
        || output.provenance.image_width != Some(width)
        || output.provenance.image_height != Some(height)
    {
        return Err(format!(
            "Little Life output '{}' has wrong dimensions",
            output.id
        ));
    }
    Ok(())
}

fn verify_little_life_transcript(root: &Path) -> Result<(), String> {
    let transcript = fs::read_to_string(root.join("presentation.txt"))
        .map_err(|error| format!("read Little Life transcript: {error}"))?;
    let mut lines = transcript.lines();
    let mut generations = Vec::new();
    while let Some(line) = lines.next() {
        if !line.starts_with("SCALAR-FIELD title=\"Orbium evolution\"") {
            continue;
        }
        let generation = line
            .split_whitespace()
            .find_map(|part| part.strip_prefix("generation="))
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or("Little Life transcript has a malformed generation")?;
        generations.push(generation);
        for _ in 0..16 {
            let row = lines
                .next()
                .ok_or("Little Life scalar-field presentation is truncated")?;
            if row.len() != 32 || !row.bytes().all(|byte| b" .:-=+*#%@".contains(&byte)) {
                return Err("Little Life scalar-field presentation has invalid cells".into());
            }
        }
    }
    if generations != (1..=32).collect::<Vec<_>>() {
        return Err(
            "Little Life transcript does not contain exactly generations 1 through 32".into(),
        );
    }
    Ok(())
}

fn verify_little_life_execution(root: &Path, identity: (&str, &str)) -> Result<(), String> {
    let report: Value = serde_json::from_slice(
        &fs::read(root.join("execution.json"))
            .map_err(|error| format!("read Little Life execution report: {error}"))?,
    )
    .map_err(|error| format!("decode Little Life execution report: {error}"))?;
    let plans = report
        .get("plans")
        .and_then(Value::as_array)
        .filter(|plans| plans.len() == 1)
        .ok_or("Little Life execution report must contain exactly one Plan")?;
    if plans[0].get("plan_id").and_then(Value::as_str) != Some(identity.0) {
        return Err("Little Life execution Plan disagrees with manifest provenance".into());
    }
    let terminal = report
        .get("observations")
        .and_then(Value::as_array)
        .and_then(|observations| {
            observations.iter().find(|observation| {
                observation
                    .pointer("/kind/PlanTerminal/disposition")
                    .and_then(Value::as_str)
                    == Some("Completed")
            })
        })
        .ok_or("Little Life execution report lacks a completed Plan terminal")?;
    if terminal.get("active_play_id").and_then(Value::as_str) != Some(identity.1) {
        return Err("Little Life execution Play disagrees with manifest provenance".into());
    }
    Ok(())
}

fn verify_hears_speaks(root: &Path, manifest: &Manifest) -> Result<(), String> {
    let expected = [
        (
            "hears-speaks.input-pcm",
            EvidenceKind::Audio,
            "audio/L16; rate=16000; channels=1",
        ),
        ("hears-speaks.input-wav", EvidenceKind::Audio, "audio/wav"),
        (
            "hears-speaks.recognition",
            EvidenceKind::MachineReadableManifest,
            "application/json",
        ),
        (
            "hears-speaks.response",
            EvidenceKind::MachineReadableManifest,
            "application/json",
        ),
        ("hears-speaks.output-wav", EvidenceKind::Audio, "audio/wav"),
        (
            "hears-speaks.receipt",
            EvidenceKind::MachineReadableManifest,
            "application/json",
        ),
    ];
    if manifest.outputs.len() != expected.len() {
        return Err("complete hears/speaks evidence must contain exactly six outputs".into());
    }
    for (id, kind, media_type) in expected {
        let output = manifest
            .outputs
            .iter()
            .find(|output| output.id == id)
            .ok_or_else(|| format!("complete hears/speaks evidence is missing '{id}'"))?;
        if !output.required
            || output.kind != kind
            || output.media_type != media_type
            || output.provenance.scenario_id != "hears-speaks.recorded-addressed-house@1"
            || output.provenance.proof_class.as_deref() != Some("hosted-recorded-audio-plan-play")
            || output.provenance.asserted_semantic_disposition.as_deref() != Some("completed")
            || [
                output.provenance.plan_id.as_deref(),
                output.provenance.active_play_id.as_deref(),
            ]
            .into_iter()
            .any(|value| value.is_none_or(str::is_empty))
        {
            return Err(format!(
                "hears/speaks output '{id}' lacks exact typed provenance"
            ));
        }
    }
    for id in ["hears-speaks.input-wav", "hears-speaks.output-wav"] {
        let output = manifest
            .outputs
            .iter()
            .find(|output| output.id == id)
            .unwrap();
        let bytes = fs::read(root.join(&output.path))
            .map_err(|error| format!("read hears/speaks WAV: {error}"))?;
        if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(format!("hears/speaks output '{id}' is not a bounded WAV"));
        }
    }
    verify_hears_speaks_providers(root)?;
    Ok(())
}

fn verify_hears_speaks_providers(root: &Path) -> Result<(), String> {
    let receipt: Value = serde_json::from_slice(
        &fs::read(root.join("receipt.json"))
            .map_err(|error| format!("read hears/speaks receipt: {error}"))?,
    )
    .map_err(|error| format!("decode hears/speaks receipt: {error}"))?;
    if receipt.pointer("/providers/schema").and_then(Value::as_str)
        != Some("conduit.journey/hears-speaks-providers@1")
    {
        return Err("hears/speaks receipt lacks provider provenance".into());
    }
    for pointer in [
        "/providers/whisper/executable_sha256",
        "/providers/whisper/model_sha256",
        "/providers/local_model/model_content_identity",
        "/providers/piper/executable_sha256",
        "/providers/piper/model_sha256",
        "/providers/piper/config_sha256",
    ] {
        let value = receipt
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or("");
        let digest = value.strip_prefix("sha256:").unwrap_or(value);
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "hears/speaks provider identity '{pointer}' is not a SHA-256 digest"
            ));
        }
    }
    for pointer in [
        "/providers/whisper/implementation",
        "/providers/local_model/runtime_version",
        "/providers/local_model/model_name",
        "/providers/piper/implementation",
    ] {
        if receipt
            .pointer(pointer)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(format!(
                "hears/speaks provider identity '{pointer}' is missing"
            ));
        }
    }
    Ok(())
}

fn verify_conduitos_console(root: &Path, manifest: &Manifest, commit: &str) -> Result<(), String> {
    if manifest.outputs.len() != 1 {
        return Err("complete ConduitOS console evidence must contain exactly one output".into());
    }
    let output = &manifest.outputs[0];
    let provenance = &output.provenance;
    let exact_sha = |value: Option<&str>| {
        value.is_some_and(|value| {
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    };
    if output.id != "conduitos.x86_64.console"
        || !output.required
        || output.kind != EvidenceKind::ConsoleTranscript
        || output.media_type != "text/plain; charset=utf-8"
        || provenance.scenario_id != "conduitos.x86_64.p5-console@1"
        || provenance.proof_class.as_deref() != Some("freestanding-emulator")
        || provenance.architecture.as_deref() != Some("x86_64")
        || provenance.architecture_rung.as_deref()
            != Some("conduitos/x86_64/P5-observatory-patchbay")
        || provenance.emulator.as_deref() != Some("qemu-system-x86_64")
        || !provenance
            .emulator_version
            .as_deref()
            .is_some_and(|value| value.starts_with("QEMU emulator version "))
        || provenance.machine.as_deref()
            != Some("q35-single-cpu-64m-headless-xhci-usb-kbd-usb-mouse-usb-ftdi-adlib")
        || !provenance
            .firmware
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || !exact_sha(provenance.host_id.as_deref())
        || !exact_sha(provenance.boot_id.as_deref())
        || provenance.kernel_artifact_id.as_deref()
            != Some(format!("conduitos-build/{}", commit.to_ascii_lowercase()).as_str())
        || !exact_sha(provenance.kernel_artifact_sha256.as_deref())
        || !provenance
            .capture_trigger
            .as_deref()
            .is_some_and(|value| value.contains("semantic-result"))
        || provenance.capture_byte_limit != Some(256 * 1024)
        || output.bytes > provenance.capture_byte_limit.unwrap_or(0)
        || provenance.image_width.is_some()
        || provenance.image_height.is_some()
        || provenance.physical_evidence != Some(false)
        || ![
            provenance.step_id.as_deref(),
            provenance.plan_id.as_deref(),
            provenance.active_play_id.as_deref(),
            provenance.asserted_semantic_disposition.as_deref(),
        ]
        .into_iter()
        .all(|value| value.is_some_and(|value| !value.trim().is_empty()))
    {
        return Err(
            "complete ConduitOS console evidence lacks exact emulator/rung/artifact provenance"
                .into(),
        );
    }
    let transcript = fs::read_to_string(root.join(&output.path))
        .map_err(|error| format!("ConduitOS console evidence is not UTF-8: {error}"))?;
    for marker in [
        "CONDUIT_BOOT_SIGN ",
        "CONDUIT_KERNEL_SIGN ",
        "CONDUIT_OBSERVATORY_SNAPSHOT ",
        "CONDUIT_SERIAL_PRESENT HELLO, CONDUITOS",
    ] {
        if transcript.matches(marker).count() != 1 {
            return Err(format!(
                "ConduitOS console evidence lacks exactly one validated marker '{marker}'"
            ));
        }
    }
    if !transcript.ends_with('\n') {
        return Err("ConduitOS console evidence is not terminal-line complete".into());
    }
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
