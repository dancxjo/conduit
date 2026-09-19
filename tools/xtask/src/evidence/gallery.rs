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
                format!(" · <a href=\"commits/{commit}/one-form-two-fronts/\">One Form, Two Fronts</a>")
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
        "\n<p><a href=\"current/one-form-two-fronts/\">Current One Form, Two Fronts journey</a></p>"
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
        "<article class=\"journey-card\"><p class=\"eyebrow\">QEMU · x86_64 · freestanding</p><h2>A computer is born</h2><p>A ConduitOS Body wakes, discovers its Host, and reaches recognizable work in one validated console run.</p><p class=\"card-boundary\">Boundary: exact QEMU machine profile; not physical hardware.</p><p><a class=\"primary\" href=\"current/conduitos/x86_64/\">Follow the evidence</a></p></article>"
    } else {
        "<!-- conduit-conduitos-journey-card@1 -->"
    };
    let body = format!(
        "<header class=\"gallery-hero\"><p class=\"eyebrow\">True stories from ordinary Conduit execution</p><h1>Follow the evidence</h1><p class=\"lede\">Begin with a recognizable moment. Then follow what Conduit was asked to do, what actually happened, and the exact evidence that makes the claim honest.</p><p class=\"boundary\"><strong>Nothing here is a simulated demo.</strong> Every displayed artifact was admitted by a verified manifest for the accepted source commit. Each story names its limits.</p></header><div class=\"cards\">{audio_card}{two_fronts_card}{little_life_card}{conduitos_card}</div><details class=\"history\"><summary>All accepted evidence and history</summary><p>Current accepted main: <code>{}</code></p>{patchbay}{conduitos}{hears_speaks}{two_fronts}{little_life}<ul>{history}</ul><p>History retains the latest {RETAINED_COMMITS} published main commits. Semantic proof remains authoritative; media are documentary evidence.</p></details>",
        escape_html(&index.current_commit)
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
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>{}</title><style>:root{{--ink:#f3f6ec;--muted:#b9c8bd;--green:#72e0aa;--gold:#f0bb67;--paper:#101714;--card:#18251f;--line:#496657}}*{{box-sizing:border-box}}html{{scroll-behavior:smooth}}body{{margin:0 auto;max-width:92rem;padding:clamp(1rem,4vw,3rem);font:16px/1.6 system-ui,sans-serif;background:radial-gradient(circle at 90% 0,#264532 0,transparent 28rem),var(--paper);color:var(--ink)}}a{{color:var(--green);text-underline-offset:.18em}}code{{overflow-wrap:anywhere}}h1{{max-width:14ch;font-size:clamp(2.8rem,7vw,6.5rem);line-height:.95;letter-spacing:-.055em;margin:.15em 0}}h2{{font-size:clamp(1.45rem,3vw,2.25rem);line-height:1.1}}img{{display:block;max-width:100%;height:auto;border:1px solid var(--line)}}figure{{margin:0}}figcaption{{font-weight:700;margin-top:.35rem}}audio{{width:100%}}button,input{{font:inherit}}nav{{margin-bottom:2rem}}.gallery-hero{{padding:clamp(2rem,8vw,7rem) 0}}.lede{{font-size:clamp(1.15rem,2.4vw,1.55rem);max-width:62rem;color:var(--muted)}}.eyebrow,.step{{color:#a8e5c4;font-weight:800;letter-spacing:.09em;text-transform:uppercase;font-size:.78rem}}.cards{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1.25rem}}.journey-card,.media-card,.life-player,.checkpoint{{padding:clamp(1.15rem,2.5vw,1.75rem);border:1px solid var(--line);border-radius:1rem;background:linear-gradient(145deg,#1a2922,var(--card))}}.journey-card img{{aspect-ratio:16/9;object-fit:cover}}.card-boundary{{color:var(--muted);font-size:.9rem}}.primary{{display:inline-block;padding:.65rem .9rem;background:var(--green);color:#102018;border-radius:.45rem;font-weight:800;text-decoration:none}}.comparison{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1.25rem}}.identity-flow,.story-arrow{{margin:1.5rem 0;padding:1rem;text-align:center;display:grid;gap:.45rem;background:var(--card);border-radius:.75rem}}.story-path{{display:grid;gap:1rem;margin:2rem 0;counter-reset:story}}.checkpoint{{position:relative;border-left:.35rem solid var(--green)}}.checkpoint h2{{margin:.2rem 0 1rem}}.checkpoint dl{{grid-template-columns:minmax(8.5rem,12rem) minmax(0,1fr)}}.boundary{{padding:1rem;border-left:.3rem solid var(--gold);background:#2a2318}}.proof-grid{{display:grid;grid-template-columns:1fr 1fr;gap:1rem}}.proof-grid>section{{padding:1rem;background:var(--card);border-radius:.75rem}}.reproduce{{padding:1rem;background:#0a110e;border:1px solid var(--line);overflow:auto}}details{{margin:1.5rem 0;padding:1rem;border:1px solid var(--line);border-radius:.6rem}}summary{{cursor:pointer;font-weight:800}}.life-player{{max-width:46rem;margin:1.5rem auto}}.life-player img{{width:100%;image-rendering:pixelated}}.controls{{display:flex;gap:1rem}}.controls input{{flex:1}}dl{{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:.4rem 1rem}}dt{{font-weight:700;color:var(--muted)}}dd{{margin:0}}@media(max-width:48rem){{.comparison,.cards,.proof-grid{{grid-template-columns:1fr}}dl,.checkpoint dl{{grid-template-columns:1fr}}}}@media(prefers-reduced-motion:reduce){{*{{scroll-behavior:auto!important;animation:none!important;transition:none!important}}}}</style></head><body>{body}</body></html>",
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
