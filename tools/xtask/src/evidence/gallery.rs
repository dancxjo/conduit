use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{
    verify, ExpectedEvidenceResult, VerificationRequest, VerifiedEvidence, VerifiedOutput,
};

mod conduitos;
mod hears_speaks;
mod little_life;
mod retention;
mod two_fronts;

use conduitos::{write_conduitos_commit, write_conduitos_current};
use hears_speaks::{write_hears_speaks_commit, write_hears_speaks_current};
use little_life::{write_little_life_commit, write_little_life_current};
use retention::{trim_indexed_history_to_bounds, validate_existing_tree};
use two_fronts::{write_two_fronts_commit, write_two_fronts_current};

const GALLERY_SCHEMA: &str = "conduit.visual-evidence-gallery/v1";
const RETAINED_COMMITS: usize = 32;
const SCENARIOS: &[(&str, &str)] = &[
    ("overview", "Overview"),
    ("selected-gear", "Selected gear"),
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
    pub evidence_root: Option<PathBuf>,
    pub conduitos_evidence_root: Option<PathBuf>,
    pub hears_speaks_evidence_root: Option<PathBuf>,
    pub two_fronts_evidence_root: Option<PathBuf>,
    pub little_life_evidence_root: Option<PathBuf>,
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
    validate_commit(&request.commit)?;
    if request.evidence_root.is_none()
        && request.conduitos_evidence_root.is_none()
        && request.hears_speaks_evidence_root.is_none()
        && request.two_fronts_evidence_root.is_none()
        && request.little_life_evidence_root.is_none()
    {
        return Err("gallery publication requires at least one verified evidence input".into());
    }
    let evidence = request
        .evidence_root
        .as_ref()
        .map(|root| {
            verify(&VerificationRequest {
                root: root.clone(),
                commit: request.commit.clone(),
                result: ExpectedEvidenceResult::Complete,
                proof_id: "browser-host".into(),
                suite_id: "prove.browser-host".into(),
            })
        })
        .transpose()?;
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
    let hears_speaks = request
        .hears_speaks_evidence_root
        .as_ref()
        .map(|root| {
            verify(&VerificationRequest {
                root: root.clone(),
                commit: request.commit.clone(),
                result: ExpectedEvidenceResult::Complete,
                proof_id: "journey-hears-speaks".into(),
                suite_id: "journey-gallery".into(),
            })
        })
        .transpose()?;
    let two_fronts = request
        .two_fronts_evidence_root
        .as_ref()
        .map(|root| {
            verify(&VerificationRequest {
                root: root.clone(),
                commit: request.commit.clone(),
                result: ExpectedEvidenceResult::Complete,
                proof_id: "journey-one-form-two-fronts".into(),
                suite_id: "journey-gallery".into(),
            })
        })
        .transpose()?;
    let little_life = request
        .little_life_evidence_root
        .as_ref()
        .map(|root| {
            verify(&VerificationRequest {
                root: root.clone(),
                commit: request.commit.clone(),
                result: ExpectedEvidenceResult::Complete,
                proof_id: "journey-little-life".into(),
                suite_id: "journey-gallery".into(),
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
    index.commits.retain(|commit| commit != &request.commit);
    index.commits.insert(0, request.commit.clone());
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
    index.current_commit = request.commit.clone();

    if let (Some(root), Some(evidence)) = (&request.evidence_root, &evidence) {
        write_commit_snapshot(&site_root, root, evidence)?;
        write_current_pages(&site_root, root, evidence)?;
    } else {
        let current = site_root.join("current/patchbay");
        if current.exists() {
            fs::remove_dir_all(current)
                .map_err(|error| format!("cannot clear stale Patchbay evidence: {error}"))?;
        }
    }
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
    if let (Some(root), Some(evidence)) = (&request.hears_speaks_evidence_root, &hears_speaks) {
        write_hears_speaks_commit(&site_root, root, evidence)?;
        write_hears_speaks_current(&site_root, root, evidence)?;
    } else {
        let current = site_root.join("current/hears-speaks");
        if current.exists() {
            fs::remove_dir_all(current).map_err(|error| {
                format!("cannot clear stale Hears and Speaks evidence: {error}")
            })?;
        }
    }
    if let (Some(root), Some(evidence)) = (&request.two_fronts_evidence_root, &two_fronts) {
        write_two_fronts_commit(&site_root, root, evidence)?;
        write_two_fronts_current(&site_root, root, evidence)?;
    } else {
        let current = site_root.join("current/one-form-two-fronts");
        if current.exists() {
            fs::remove_dir_all(current)
                .map_err(|error| format!("cannot clear stale Two Fronts evidence: {error}"))?;
        }
    }
    if let (Some(root), Some(evidence)) = (&request.little_life_evidence_root, &little_life) {
        write_little_life_commit(&site_root, root, evidence)?;
        write_little_life_current(&site_root, root, evidence)?;
    } else {
        let current = site_root.join("current/little-life");
        if current.exists() {
            fs::remove_dir_all(current)
                .map_err(|error| format!("cannot clear stale Little Life evidence: {error}"))?;
        }
    }
    fs::write(site_root.join(".nojekyll"), b"")
        .map_err(|error| format!("cannot write gallery marker: {error}"))?;
    write_gallery_index(&site_root, &index, conduitos.is_some())?;
    trim_indexed_history_to_bounds(&site_root, &mut index, conduitos.is_some())?;
    println!(
        "published gallery source for {} with {} retained commits",
        request.commit,
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
                format!(" · <a href=\"commits/{commit}/conduitos/x86_64/\">ConduitOS x86_64</a>")
            } else {
                String::new()
            };
            let hears_speaks = if root
                .join("commits")
                .join(commit)
                .join("hears-speaks/index.html")
                .is_file()
            {
                format!(" · <a href=\"commits/{commit}/hears-speaks/\">Hears and Speaks</a>")
            } else {
                String::new()
            };
            let two_fronts = if root
                .join("commits")
                .join(commit)
                .join("one-form-two-fronts/index.html")
                .is_file()
            {
                format!(" · <a href=\"commits/{commit}/one-form-two-fronts/\">One form, Two Fronts</a>")
            } else {
                String::new()
            };
            let little_life = if root
                .join("commits")
                .join(commit)
                .join("little-life/index.html")
                .is_file()
            {
                format!(" · <a href=\"commits/{commit}/little-life/\">Little Life</a>")
            } else {
                String::new()
            };
            let patchbay = if root
                .join("commits")
                .join(commit)
                .join("patchbay/index.html")
                .is_file()
            {
                format!(" · <a href=\"commits/{commit}/patchbay/\">Patchbay</a>")
            } else {
                String::new()
            };
            format!(
                "<li><code>{commit}</code>{patchbay}{conduitos}{hears_speaks}{two_fronts}{little_life}</li>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let conduitos = if has_conduitos {
        "\n<p><a href=\"current/conduitos/x86_64/\">Current x86_64 ConduitOS emulator console evidence</a></p>"
    } else {
        ""
    };
    let hears_speaks = if root.join("current/hears-speaks/index.html").is_file() {
        "\n<p><a href=\"current/hears-speaks/\">Current Hears and Speaks audio journey</a></p>"
    } else {
        ""
    };
    let two_fronts = if root
        .join("current/one-form-two-fronts/index.html")
        .is_file()
    {
        "\n<p><a href=\"current/one-form-two-fronts/\">Current One form, Two Fronts journey</a></p>"
    } else {
        ""
    };
    let little_life = if root.join("current/little-life/index.html").is_file() {
        "\n<p><a href=\"current/little-life/\">Current Little Life evolution journey</a></p>"
    } else {
        ""
    };
    let patchbay = if root.join("current/patchbay/index.html").is_file() {
        "\n<p><a href=\"current/patchbay/\">Current Patchbay evidence</a></p>"
    } else {
        ""
    };
    let audio_card = if root.join("current/hears-speaks/index.html").is_file() {
        "<article class=\"journey-card audio\"><p class=\"eyebrow\">Recorded audio · local providers</p><h2>It hears and speaks</h2><p>Hear one admitted recording become recognized language, an addressed answer, and a newly synthesized WAV.</p><audio controls preload=\"metadata\" src=\"current/hears-speaks/output.wav\"></audio><p class=\"card-boundary\">Boundary: hosted provider execution over recorded audio; not a live microphone or speaker claim.</p><p><a class=\"primary\" href=\"current/hears-speaks/\">Follow the evidence</a></p></article>"
    } else {
        ""
    };
    let two_fronts_card = if root
        .join("current/one-form-two-fronts/index.html")
        .is_file()
    {
        "<article class=\"journey-card\"><p class=\"eyebrow\">Pinned Chromium + native software renderer</p><h2>One meaning, two fronts</h2><img src=\"current/one-form-two-fronts/browser.png\" alt=\"Morse Network manifested in a browser\"><p>The same semantic Presentation crossed two rendering boundaries without changing identity.</p><p class=\"card-boundary\">Boundary: software-rendered native pixels and pinned Chromium; not physical display proof.</p><p><a class=\"primary\" href=\"current/one-form-two-fronts/\">Follow the evidence</a></p></article>"
    } else {
        ""
    };
    let little_life_card = if root.join("current/little-life/index.html").is_file() {
        "<article class=\"journey-card\"><p class=\"eyebrow\">Deterministic hosted execution</p><h2>A tiny world lives</h2><img src=\"current/little-life/t032.png\" alt=\"Orbium scalar field at generation 32\"><p>A bounded seed changes through 32 real Plan/Play generations. Four accepted moments tell the story.</p><p class=\"card-boundary\">Boundary: semantic scalar-field output; not a native display or physical observation.</p><p><a class=\"primary\" href=\"current/little-life/\">See what happened</a></p></article>"
    } else {
        ""
    };
    let conduitos_card = if has_conduitos {
        "<article class=\"journey-card\"><p class=\"eyebrow\">QEMU · x86_64 · freestanding</p><h2>A computer is born</h2><p>A ConduitOS Body wakes, discovers its host, and reaches recognizable work in one validated console run.</p><p class=\"card-boundary\">Boundary: exact QEMU machine profile; not physical hardware.</p><p><a class=\"primary\" href=\"current/conduitos/x86_64/\">Follow the evidence</a></p></article>"
    } else {
        "<!-- conduit-conduitos-journey-card@1 -->"
    };
    let body = format!(
        "<header class=\"gallery-hero\"><p class=\"eyebrow\">Conduit's flagship proof</p><h1>One Journey.<br><em>Three Bodies.</em></h1><p class=\"lede\">One portable meaning, lived independently through radically different machinery. Follow a Body from birth to fulfillment—or turn the view sideways and compare the same semantic moment across all three.</p><div class=\"thesis\" aria-label=\"The Conduit thesis\"><span>Meaning stays</span><i aria-hidden=\"true\">→</i><span>machinery changes</span><i aria-hidden=\"true\">→</i><span>truth remains exact</span></div></header><main><!-- conduit-three-body-flagship@2 --><section class=\"flagship awaiting\" aria-labelledby=\"flagship-title\"><div><p class=\"eyebrow\">The shared semantic spine</p><h2 id=\"flagship-title\">Birth to fulfillment, three times honestly</h2><p class=\"lede\">The publication appears here only when three independently verified biographies belong to this exact accepted commit.</p></div><div class=\"body-lanes\"><article><b>A</b><h3>ConduitOS</h3><p>Native, freestanding, graphical</p></article><article><b>B</b><h3>Browser</h3><p>DOM, WASM, interactive</p></article><article><b>C</b><h3>Distributed</h3><p>Multi-Host, Line, generative</p></article></div><ol class=\"semantic-spine\"><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change</li><li>Replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol><p class=\"boundary\"><strong>Evidence not yet admitted for this commit.</strong> No neighboring proof is promoted to fill an empty track.</p></section><!-- conduit-three-body-flagship:end --><section class=\"evidence-library\" aria-labelledby=\"library-title\"><p class=\"eyebrow\">The evidence library</p><h2 id=\"library-title\">Other true stories</h2><p class=\"section-intro\">Smaller proofs of particular boundaries. Each says exactly what happened—and what did not.</p><section class=\"cards\">{audio_card}{two_fronts_card}{little_life_card}{conduitos_card}</section></section><details class=\"history\"><summary>Provenance, accepted evidence, and history</summary><p>Current accepted main: <code>{}</code></p>{patchbay}{conduitos}{hears_speaks}{two_fronts}{little_life}<ul>{history}</ul><p>History retains the latest {RETAINED_COMMITS} published main commits. Semantic proof remains authoritative; media are documentary evidence.</p></details></main>",
        escape_html(&index.current_commit)
    );
    let body = body.replace(
        "<ol class=\"semantic-spine\"><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change</li><li>Replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol>",
        "<ol class=\"semantic-spine\"><li>Before</li><li>Bootstrap</li><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change / replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol>",
    );
    write_html(&root.join("index.html"), "Conduit evidence gallery", &body)
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
        ("Active play", optional(&provenance.active_play_id)),
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
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>{}</title><style>:root{{--ink:#f5f2e8;--muted:#afbbb3;--green:#77e6ad;--gold:#f5b95f;--coral:#ff806c;--blue:#73b9ff;--paper:#090e0c;--card:#131d18;--line:#31473c}}*{{box-sizing:border-box}}html{{scroll-behavior:smooth}}body{{margin:0 auto;max-width:96rem;padding:clamp(1rem,4vw,4rem);font:16px/1.6 ui-sans-serif,system-ui,sans-serif;background:radial-gradient(circle at 82% 3%,#193d2b 0,transparent 27rem),radial-gradient(circle at 8% 24%,#252016 0,transparent 24rem),var(--paper);color:var(--ink)}}a{{color:var(--green);text-underline-offset:.2em}}code{{overflow-wrap:anywhere}}h1{{max-width:13ch;font-size:clamp(3.5rem,9vw,8.5rem);line-height:.82;letter-spacing:-.07em;margin:.16em 0}}h1 em{{color:var(--green);font-style:normal}}h2{{font-size:clamp(2rem,4vw,4rem);line-height:1;letter-spacing:-.035em;margin:.25em 0}}h3{{font-size:1.35rem;margin:.3rem 0}}img{{display:block;max-width:100%;height:auto;border:1px solid var(--line)}}figure{{margin:0}}figcaption{{font-weight:700;margin-top:.35rem}}audio{{width:100%}}button,input{{font:inherit}}nav{{margin-bottom:2rem}}.gallery-hero{{min-height:72vh;display:flex;flex-direction:column;justify-content:center;padding:clamp(3rem,10vw,9rem) 0}}.lede{{font-size:clamp(1.15rem,2.4vw,1.65rem);max-width:54rem;color:var(--muted)}}.eyebrow,.step{{color:#a8e5c4;font-weight:850;letter-spacing:.14em;text-transform:uppercase;font-size:.76rem}}.thesis{{display:flex;flex-wrap:wrap;gap:.8rem 1.3rem;align-items:center;margin-top:2.5rem;color:var(--muted);font-weight:700}}.thesis i{{color:var(--gold);font-style:normal}}.flagship{{padding:clamp(1.4rem,4vw,4rem);border:1px solid var(--green);border-radius:1.5rem;background:linear-gradient(145deg,#15281fdd,#101713ee);box-shadow:0 2rem 7rem #0008;margin-bottom:clamp(5rem,10vw,10rem)}}.flagship.awaiting{{border-color:#7b6745}}.body-lanes{{display:grid;grid-template-columns:repeat(3,1fr);gap:1rem;margin:2.5rem 0}}.body-lanes article{{min-height:12rem;padding:1.3rem;border:1px solid var(--line);border-radius:1rem;background:#0c1410;position:relative;overflow:hidden}}.body-lanes article::after{{content:\"\";position:absolute;inset:auto -20% -50% 25%;height:9rem;background:radial-gradient(circle,var(--green),transparent 68%);opacity:.18}}.body-lanes article:nth-child(2)::after{{background:radial-gradient(circle,var(--blue),transparent 68%)}}.body-lanes article:nth-child(3)::after{{background:radial-gradient(circle,var(--coral),transparent 68%)}}.body-lanes b{{display:grid;place-items:center;width:2.2rem;height:2.2rem;border:1px solid var(--green);border-radius:50%;color:var(--green)}}.body-lanes p{{color:var(--muted)}}.semantic-spine{{display:grid;grid-template-columns:repeat(12,minmax(4.4rem,1fr));gap:.35rem;padding:0;overflow:auto;list-style:none;counter-reset:moment}}.semantic-spine li{{counter-increment:moment;min-width:4.4rem;padding:.65rem .35rem;border-top:2px solid var(--green);color:var(--muted);font-size:.76rem}}.semantic-spine li::before{{content:counter(moment,decimal-leading-zero);display:block;color:var(--green);font-weight:800}}.evidence-library{{margin-bottom:5rem}}.section-intro{{max-width:45rem;color:var(--muted)}}.cards{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1.25rem;margin-top:2rem}}.journey-card,.media-card,.life-player,.checkpoint{{padding:clamp(1.15rem,2.5vw,1.75rem);border:1px solid var(--line);border-radius:1rem;background:linear-gradient(145deg,#17231d,var(--card))}}.journey-card img{{aspect-ratio:16/9;object-fit:cover}}.card-boundary{{color:var(--muted);font-size:.9rem}}.primary{{display:inline-block;padding:.7rem 1rem;background:var(--green);color:#08140e;border-radius:999px;font-weight:850;text-decoration:none}}.comparison{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1.25rem}}.identity-flow,.story-arrow{{margin:1.5rem 0;padding:1rem;text-align:center;display:grid;gap:.45rem;background:var(--card);border-radius:.75rem}}.story-path{{display:grid;gap:1rem;margin:2rem 0;counter-reset:story}}.checkpoint{{position:relative;border-left:.35rem solid var(--green)}}.checkpoint h2{{margin:.2rem 0 1rem}}.checkpoint dl{{grid-template-columns:minmax(8.5rem,12rem) minmax(0,1fr)}}.boundary{{padding:1rem;border-left:.3rem solid var(--gold);background:#2a2318}}.proof-grid{{display:grid;grid-template-columns:1fr 1fr;gap:1rem}}.proof-grid>section{{padding:1rem;background:var(--card);border-radius:.75rem}}.reproduce{{padding:1rem;background:#0a110e;border:1px solid var(--line);overflow:auto}}details{{margin:1.5rem 0;padding:1rem;border:1px solid var(--line);border-radius:.8rem}}summary{{cursor:pointer;font-weight:800}}.life-player{{max-width:46rem;margin:1.5rem auto}}.life-player img{{width:100%;image-rendering:pixelated}}.controls{{display:flex;gap:1rem}}.controls input{{flex:1}}dl{{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:.4rem 1rem}}dt{{font-weight:700;color:var(--muted)}}dd{{margin:0}}@media(max-width:48rem){{.comparison,.cards,.proof-grid,.body-lanes{{grid-template-columns:1fr}}.body-lanes article{{min-height:8rem}}dl,.checkpoint dl{{grid-template-columns:1fr}}h1{{font-size:clamp(3.2rem,18vw,5.5rem)}}}}@media(prefers-reduced-motion:reduce){{*{{scroll-behavior:auto!important;animation:none!important;transition:none!important}}}}</style></head><body>{body}</body></html>",
        escape_html(title)
    )
    .replace(
        "grid-template-columns:repeat(12,minmax(4.4rem,1fr))",
        "grid-template-columns:repeat(13,minmax(4.4rem,1fr))",
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
