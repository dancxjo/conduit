use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{
    verify, ExpectedEvidenceResult, VerificationRequest, VerifiedEvidence, VerifiedOutput,
};

mod conduitos;
mod retention;

use conduitos::{write_conduitos_commit, write_conduitos_current};
use retention::{trim_indexed_history_to_bounds, validate_existing_tree};

const GALLERY_SCHEMA: &str = "conduit.visual-evidence-gallery/v1";
const RETAINED_COMMITS: usize = 32;
const SCENARIOS: &[(&str, &str)] = &[
    ("overview", "Overview"),
    ("selected-gear", "Selected Gear"),
    ("plan-lens", "Plan lens"),
    ("play-lens", "Play lens"),
    ("signs-lens", "Signs lens"),
    ("route-recovery", "Route recovery"),
    ("interaction", "Interaction"),
    ("high-contrast", "High contrast"),
    ("disconnected", "Disconnected and retained"),
    ("responsive", "Responsive enlarged content"),
];

pub struct GalleryRequest {
    pub evidence_root: PathBuf,
    pub conduitos_evidence_root: Option<PathBuf>,
    pub site_root: PathBuf,
    pub commit: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GalleryIndex {
    schema: String,
    current_commit: String,
    retention_commits: usize,
    commits: Vec<String>,
}

pub fn publish_gallery(request: &GalleryRequest) -> Result<(), String> {
    let evidence = verify(&VerificationRequest {
        root: request.evidence_root.clone(),
        commit: request.commit.clone(),
        result: ExpectedEvidenceResult::Complete,
        proof_id: "browser-host".into(),
        suite_id: "prove.browser-host".into(),
    })?;
    let conduitos = request
        .conduitos_evidence_root
        .as_ref()
        .map(|root| {
            verify(&VerificationRequest {
                root: root.clone(),
                commit: request.commit.clone(),
                result: ExpectedEvidenceResult::Complete,
                proof_id: "conduitos-x86_64".into(),
                suite_id: "conduitos.prove.x86_64".into(),
            })
        })
        .transpose()?;
    fs::create_dir_all(&request.site_root).map_err(|error| {
        format!(
            "cannot create gallery root {}: {error}",
            request.site_root.display()
        )
    })?;
    reject_symlink_root(&request.site_root)?;
    let site_root = request.site_root.canonicalize().map_err(|error| {
        format!(
            "cannot resolve gallery root {}: {error}",
            request.site_root.display()
        )
    })?;
    let mut index = load_index(&site_root)?;
    validate_existing_tree(&site_root, &index)?;
    index.commits.retain(|commit| commit != &evidence.commit);
    index.commits.insert(0, evidence.commit.clone());
    let evicted = index
        .commits
        .split_off(index.commits.len().min(RETAINED_COMMITS));
    for commit in evicted {
        validate_commit(&commit)?;
        let path = site_root.join("commits").join(commit);
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("cannot evict gallery history: {error}"))?;
        }
    }
    index.current_commit = evidence.commit.clone();

    write_commit_snapshot(&site_root, &request.evidence_root, &evidence)?;
    write_current_pages(&site_root, &request.evidence_root, &evidence)?;
    if let (Some(root), Some(conduitos)) = (&request.conduitos_evidence_root, &conduitos) {
        write_conduitos_commit(&site_root, root, conduitos)?;
        write_conduitos_current(&site_root, root, conduitos)?;
    } else {
        let current = site_root.join("current/conduitos");
        if current.exists() {
            fs::remove_dir_all(current).map_err(|error| {
                format!("cannot clear stale ConduitOS current evidence: {error}")
            })?;
        }
    }
    fs::write(site_root.join(".nojekyll"), b"")
        .map_err(|error| format!("cannot write gallery marker: {error}"))?;
    write_gallery_index(&site_root, &index, conduitos.is_some())?;
    trim_indexed_history_to_bounds(&site_root, &mut index, conduitos.is_some())?;
    println!(
        "published gallery source for {} with {} retained commits",
        evidence.commit,
        index.commits.len()
    );
    Ok(())
}

fn load_index(root: &Path) -> Result<GalleryIndex, String> {
    let path = root.join("gallery.json");
    if !path.exists() {
        return Ok(GalleryIndex {
            schema: GALLERY_SCHEMA.into(),
            current_commit: String::new(),
            retention_commits: RETAINED_COMMITS,
            commits: Vec::new(),
        });
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("cannot inspect existing gallery index: {error}"))?;
    if !metadata.file_type().is_file() || metadata.len() > 64 * 1024 {
        return Err("existing gallery index is not one bounded regular file".into());
    }
    let index: GalleryIndex = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("cannot read gallery index: {error}"))?,
    )
    .map_err(|error| format!("invalid gallery index: {error}"))?;
    if index.schema != GALLERY_SCHEMA
        || index.retention_commits != RETAINED_COMMITS
        || index.commits.len() > RETAINED_COMMITS
    {
        return Err("existing gallery index violates its schema or retention bound".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for commit in &index.commits {
        validate_commit(commit)?;
        if !seen.insert(commit) {
            return Err("existing gallery index contains duplicate commits".into());
        }
    }
    if !index.commits.is_empty() && index.commits.first() != Some(&index.current_commit) {
        return Err("existing gallery current commit is not its newest retained commit".into());
    }
    Ok(index)
}

pub(super) fn write_gallery_index(
    root: &Path,
    index: &GalleryIndex,
    has_conduitos: bool,
) -> Result<(), String> {
    write_root_index(root, index, has_conduitos)?;
    write_json(&root.join("gallery.json"), index)
}

fn write_commit_snapshot(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    let commit_root = site_root.join("commits").join(&evidence.commit);
    if commit_root.exists() {
        fs::remove_dir_all(&commit_root)
            .map_err(|error| format!("cannot replace exact gallery snapshot: {error}"))?;
    }
    let patchbay_root = commit_root.join("patchbay");
    fs::create_dir_all(&patchbay_root)
        .map_err(|error| format!("cannot create commit gallery: {error}"))?;
    copy_file(
        &evidence_root.join("manifest.json"),
        &commit_root.join("manifest.json"),
    )?;
    let declarations = required_output(evidence, "patchbay.capture-declarations")?;
    copy_file(
        &evidence_root.join(&declarations.path),
        &commit_root.join("captures.json"),
    )?;
    for (scenario, label) in SCENARIOS {
        let output = required_output(evidence, &format!("patchbay.{scenario}"))?;
        copy_file(
            &evidence_root.join(&output.path),
            &patchbay_root.join(format!("{scenario}.png")),
        )?;
        write_scenario_page(
            &patchbay_root.join(scenario).join("index.html"),
            label,
            evidence,
            output,
            &format!("../{scenario}.png"),
            "../../../../index.html",
        )?;
    }
    write_scenario_index(
        &patchbay_root.join("index.html"),
        "Accepted Patchbay evidence",
        evidence,
        "../../../index.html",
        "../manifest.json",
    )
}

fn write_current_pages(
    root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    let current_root = root.join("current/patchbay");
    if current_root.exists() {
        fs::remove_dir_all(&current_root)
            .map_err(|error| format!("cannot replace current gallery: {error}"))?;
    }
    fs::create_dir_all(&current_root)
        .map_err(|error| format!("cannot create current gallery: {error}"))?;
    for (scenario, label) in SCENARIOS {
        let output = required_output(evidence, &format!("patchbay.{scenario}"))?;
        copy_file(
            &evidence_root.join(&output.path),
            &current_root.join(format!("{scenario}.png")),
        )?;
        write_scenario_page(
            &current_root.join(scenario).join("index.html"),
            label,
            evidence,
            output,
            &format!("../{scenario}.png"),
            "../../../index.html",
        )?;
    }
    write_scenario_index(
        &current_root.join("index.html"),
        "Current accepted Patchbay",
        evidence,
        "../../index.html",
        &format!("../../commits/{}/manifest.json", evidence.commit),
    )
}

fn write_root_index(root: &Path, index: &GalleryIndex, has_conduitos: bool) -> Result<(), String> {
    let history = index
        .commits
        .iter()
        .map(|commit| {
            let conduitos = if root
                .join("commits")
                .join(commit)
                .join("conduitos/x86_64/index.html")
                .is_file()
            {
                format!(
                    " · <a href=\"commits/{commit}/conduitos/x86_64/\">ConduitOS x86_64</a>"
                )
            } else {
                String::new()
            };
            format!(
                "<li><code>{commit}</code>: <a href=\"commits/{commit}/patchbay/\">Patchbay</a>{conduitos}</li>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let conduitos = if has_conduitos {
        "\n<p><a href=\"current/conduitos/x86_64/\">Current x86_64 ConduitOS emulator console evidence</a></p>"
    } else {
        ""
    };
    let body = format!(
        "<h1>Conduit visual evidence</h1>\n<p>Current accepted main: <code>{}</code></p>\n<p><a href=\"current/patchbay/\">Current Patchbay evidence</a></p>{conduitos}\n<h2>Retained accepted commits</h2>\n<ul>{history}</ul>\n<p>History retains the latest {RETAINED_COMMITS} published main commits. Semantic proof remains authoritative; these images and transcripts are documentary evidence.</p>",
        escape_html(&index.current_commit)
    );
    write_html(&root.join("index.html"), "Conduit visual evidence", &body)
}

fn write_scenario_index(
    path: &Path,
    heading: &str,
    evidence: &VerifiedEvidence,
    home: &str,
    manifest: &str,
) -> Result<(), String> {
    let links = SCENARIOS
        .iter()
        .map(|(scenario, label)| format!("<li><a href=\"{scenario}/\">{label}</a></li>"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<h1>{heading}</h1>\n<p>Exact accepted commit: <code>{}</code></p>\n<ul>{links}</ul>\n<p><a href=\"{manifest}\">Versioned evidence manifest</a></p>",
        evidence.commit
    );
    write_html(path, heading, &body)
}

fn write_scenario_page(
    path: &Path,
    label: &str,
    evidence: &VerifiedEvidence,
    output: &VerifiedOutput,
    image_source: &str,
    home: &str,
) -> Result<(), String> {
    let provenance = &output.provenance;
    let kind = format!("{:?}", output.kind);
    let bytes = output.bytes.to_string();
    let rows = [
        ("Proof", evidence.proof_id.as_str()),
        ("Suite", evidence.suite_id.as_str()),
        ("Commit", evidence.commit.as_str()),
        ("Scenario", provenance.scenario_id.as_str()),
        ("Evidence kind", kind.as_str()),
        ("Media type", output.media_type.as_str()),
        ("Bytes", bytes.as_str()),
        ("Browser", optional(&provenance.browser_engine)),
        ("Browser version", optional(&provenance.browser_version)),
        ("Viewport", optional(&provenance.viewport)),
        ("SHA-256", output.sha256.as_str()),
        ("Plan", optional(&provenance.plan_id)),
        ("Active Play", optional(&provenance.active_play_id)),
        ("Presentation", optional(&provenance.presentation_id)),
        (
            "Presentation revision",
            optional(&provenance.presentation_revision),
        ),
        ("Manifestation", optional(&provenance.manifestation_id)),
        ("Renderer", optional(&provenance.renderer_id)),
        (
            "Asserted disposition",
            optional(&provenance.asserted_semantic_disposition),
        ),
    ]
    .into_iter()
    .map(|(name, value)| {
        format!(
            "<dt>{}</dt><dd><code>{}</code></dd>",
            escape_html(name),
            escape_html(value)
        )
    })
    .collect::<Vec<_>>()
    .join("\n");
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a> · <a href=\"../\">Patchbay scenarios</a></nav>\n<h1>{}</h1>\n<p>Documentary evidence captured only after semantic assertions passed.</p>\n<img src=\"{image_source}\" alt=\"{} for accepted Conduit commit {}\">\n<h2>Exact provenance</h2>\n<dl>{rows}</dl>",
        escape_html(label),
        escape_html(label),
        evidence.commit
    );
    write_html(path, label, &body)
}

pub(super) fn required_output<'a>(
    evidence: &'a VerifiedEvidence,
    identity: &str,
) -> Result<&'a VerifiedOutput, String> {
    evidence
        .outputs
        .iter()
        .find(|output| output.id == identity)
        .ok_or_else(|| format!("verified evidence lost required output '{identity}'"))
}

pub(super) fn write_html(path: &Path, title: &str, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create gallery page directory: {error}"))?;
    }
    let document = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>body{{margin:2rem auto;max-width:92rem;padding:0 1rem;font:16px/1.5 system-ui,sans-serif;background:#101714;color:#e8f5ed}}a{{color:#70e0aa}}code{{overflow-wrap:anywhere}}img{{display:block;max-width:100%;height:auto;border:1px solid #466455}}dl{{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:.4rem 1rem}}dt{{font-weight:700}}dd{{margin:0}}</style></head><body>{body}</body></html>",
        escape_html(title)
    );
    fs::write(path, document).map_err(|error| format!("cannot write gallery page: {error}"))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize gallery index: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write gallery index: {error}"))
}

pub(super) fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination).map_err(|error| {
        format!(
            "cannot copy gallery evidence {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn reject_symlink_root(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("cannot inspect gallery root: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("gallery root must be one real directory".into());
    }
    Ok(())
}

pub(super) fn validate_commit(value: &str) -> Result<(), String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("gallery history contains a non-exact commit identity".into())
    }
}

pub(super) fn optional(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("not recorded")
}

pub(super) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
