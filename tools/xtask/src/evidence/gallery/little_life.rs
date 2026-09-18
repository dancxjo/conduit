//! Static pages for one verified bounded Orbium/Lenia journey.

use std::path::Path;

use super::{copy_file, required_output, write_html};
use crate::evidence::VerifiedEvidence;

const FILES: &[(&str, &str)] = &[
    ("little-life.t0", "t000.png"),
    ("little-life.t1", "t001.png"),
    ("little-life.t8", "t008.png"),
    ("little-life.t32", "t032.png"),
    ("little-life.presentation", "presentation.txt"),
    ("little-life.execution", "execution.json"),
];

pub(super) fn write_little_life_commit(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root
            .join("commits")
            .join(&evidence.commit)
            .join("little-life"),
        evidence_root,
        evidence,
        "../../../index.html",
    )
}

pub(super) fn write_little_life_current(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root.join("current/little-life"),
        evidence_root,
        evidence,
        "../../index.html",
    )
}

fn write_exhibit(
    destination: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
    home: &str,
) -> Result<(), String> {
    if destination.exists() {
        std::fs::remove_dir_all(destination)
            .map_err(|error| format!("cannot replace Little Life exhibit: {error}"))?;
    }
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create Little Life exhibit: {error}"))?;
    for (id, filename) in FILES {
        let output = required_output(evidence, id)?;
        copy_file(
            &evidence_root.join(&output.path),
            &destination.join(filename),
        )?;
    }
    copy_file(
        &evidence_root.join("manifest.json"),
        &destination.join("manifest.json"),
    )?;
    let frames = [("t000.png", "Generation 0"), ("t001.png", "Generation 1"), ("t008.png", "Generation 8"), ("t032.png", "Generation 32")]
        .into_iter()
        .map(|(path, label)| format!("<figure><img src=\"{path}\" alt=\"Orbium scalar field at {label}\"><figcaption>{label}</figcaption></figure>"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "<nav><a href=\"{home}\">All journeys</a></nav>\n<p class=\"eyebrow\">A tiny world in motion</p><h1>Little Life</h1><p class=\"lede\">A deterministic Orbium seed entered one bounded Lenia Play and changed for 32 generations.</p><section class=\"life-player\" aria-label=\"Little Life accepted checkpoint player\"><img id=\"life-frame\" src=\"t000.png\" alt=\"Orbium scalar field at generation 0\"><p id=\"life-label\" aria-live=\"polite\">Generation 0</p><div class=\"controls\"><button id=\"life-toggle\" type=\"button\">Play accepted checkpoints</button><input id=\"life-scrub\" type=\"range\" min=\"0\" max=\"3\" value=\"0\" aria-label=\"Accepted generation checkpoint\"></div></section><noscript><div class=\"comparison\">{frames}</div></noscript><script>(()=>{{const f=[[\"t000.png\",0],[\"t001.png\",1],[\"t008.png\",8],[\"t032.png\",32]],i=document.querySelector(\"#life-frame\"),l=document.querySelector(\"#life-label\"),s=document.querySelector(\"#life-scrub\"),b=document.querySelector(\"#life-toggle\"),r=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;let n=0,t=null;const show=x=>{{n=Number(x);i.src=f[n][0];i.alt=`Orbium scalar field at generation ${{f[n][1]}}`;l.textContent=`Generation ${{f[n][1]}}`;s.value=n}},stop=()=>{{clearInterval(t);t=null;b.textContent=r?\"Next accepted checkpoint\":\"Play accepted checkpoints\"}};s.addEventListener(\"input\",()=>{{stop();show(s.value)}});b.addEventListener(\"click\",()=>{{if(r)return show((n+1)%f.length);if(t)return stop();b.textContent=\"Pause\";t=setInterval(()=>show((n+1)%f.length),900)}});if(r)b.textContent=\"Next accepted checkpoint\";}})()</script><p class=\"boundary\"><strong>Four accepted artifacts, no invented frames.</strong> The player moves only among retained outputs from the same evidence run; it never interpolates runtime behavior.</p><section class=\"story-path\" aria-label=\"Evidence story\"><article class=\"checkpoint\"><p class=\"step\">1 · A bounded beginning</p><h2>The seed entered the Play</h2><dl><dt>What you see</dt><dd>The exact generation-zero scalar field.</dd><dt>What happened</dt><dd>A reviewed Little Life Form admitted its deterministic seed and finite 32-step execution.</dd><dt>What Conduit established</dt><dd>The initial field and execution identities were fixed before evolution began.</dd><dt>Concepts in view</dt><dd>Form · Plan · Play · Presentation</dd><dt>Evidence</dt><dd><a href=\"t000.png\">Generation 0 artifact</a> · <a href=\"execution.json\">execution report</a></dd></dl></article><article class=\"checkpoint\"><p class=\"step\">2 · State really changed</p><h2>One Play reached generation 32</h2><dl><dt>What you see</dt><dd>Accepted generations 1, 8, and 32 in their true execution order.</dd><dt>What happened</dt><dd>The installed scalar-field terminal emitted each retained semantic checkpoint.</dd><dt>What Conduit established</dt><dd>The terminal report completed the same bounded Plan and Play that produced the frames.</dd><dt>Concepts in view</dt><dd>Play · semantic state · Presentation</dd><dt>Evidence</dt><dd><a href=\"presentation.txt\">Complete terminal presentation</a> · <a href=\"manifest.json\">manifest correlation</a></dd></dl></article></section><div class=\"proof-grid\"><section><h2>What this proves</h2><p>A deterministic seed evolved through one ordinary bounded 32-generation Conduit Plan/Play.</p></section><section><h2>What it does not prove</h2><p>This is semantic scalar-field presentation, not a native graphical renderer, physical display, or human-perception claim.</p></section></div><h2>Reproduce</h2><pre class=\"reproduce\"><code>cargo xtask evidence little-life</code></pre><details><summary>All evidence and exact execution</summary><p>Accepted commit: <code>{}</code></p><ul><li><a href=\"presentation.txt\">Complete terminal presentation</a></li><li><a href=\"execution.json\">Plan/Play execution report</a></li><li><a href=\"manifest.json\">Digest-bound manifest</a></li></ul></details>",
        evidence.commit
    );
    write_html(&destination.join("index.html"), "Little Life", &body)
}
