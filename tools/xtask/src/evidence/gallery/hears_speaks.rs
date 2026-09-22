//! Static pages for one verified hosted audio journey.

use std::path::Path;

use super::{copy_file, escape_html, required_output, write_html};
use crate::evidence::VerifiedEvidence;

const FILES: &[(&str, &str)] = &[
    ("hears-speaks.input-pcm", "input.pcm"),
    ("hears-speaks.input-wav", "input.wav"),
    ("hears-speaks.recognition", "recognition.json"),
    ("hears-speaks.response", "response.json"),
    ("hears-speaks.output-wav", "output.wav"),
    ("hears-speaks.receipt", "receipt.json"),
];

pub(super) fn write_hears_speaks_commit(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root
            .join("commits")
            .join(&evidence.commit)
            .join("hears-speaks"),
        evidence_root,
        evidence,
        "../../../index.html",
    )
}

pub(super) fn write_hears_speaks_current(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root.join("current/hears-speaks"),
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
            .map_err(|error| format!("cannot replace Hears and Speaks exhibit: {error}"))?;
    }
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create Hears and Speaks exhibit: {error}"))?;
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
    let recognition: serde_json::Value = serde_json::from_slice(
        &std::fs::read(evidence_root.join("recognition.json"))
            .map_err(|error| format!("read recognition receipt: {error}"))?,
    )
    .map_err(|error| format!("decode recognition receipt: {error}"))?;
    let response: serde_json::Value = serde_json::from_slice(
        &std::fs::read(evidence_root.join("response.json"))
            .map_err(|error| format!("read response receipt: {error}"))?,
    )
    .map_err(|error| format!("decode response receipt: {error}"))?;
    let recognized = escape_html(
        recognition
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    );
    let answered = escape_html(
        response
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    );
    let body = format!(
        "<nav><a href=\"{home}\">All journeys</a></nav>\n<p class=\"eyebrow\">A question became an answer</p><h1>Conduit Hears and Speaks</h1>\n<p class=\"lede\">One admitted recording travelled through recognition, an addressed House, and speech synthesis—inside one correlated Play.</p><section class=\"story-path\" aria-label=\"Evidence story\"><article class=\"checkpoint media-card\"><p class=\"step\">1 · A real bounded recording entered</p><h2>What Conduit heard</h2><audio controls preload=\"metadata\" src=\"input.wav\"></audio><blockquote>“{recognized}”</blockquote><dl><dt>What you see</dt><dd>A playable WAV and the exact recognized text.</dd><dt>What happened</dt><dd>The host admitted bounded PCM and the discovered Whisper provider produced the recognition receipt.</dd><dt>What Conduit established</dt><dd>The audio artifact and recognition share the journey's Plan/Play provenance.</dd><dt>Concepts in view</dt><dd>Host · Plan · Play · Info</dd><dt>Evidence</dt><dd><a href=\"input.wav\">Input WAV</a> · <a href=\"recognition.json\">recognition receipt</a></dd></dl></article><div class=\"story-arrow\" aria-hidden=\"true\">recording → Whisper → addressed House → response → Piper → WAV</div><article class=\"checkpoint media-card\"><p class=\"step\">2 · The House answered</p><h2>What Conduit said</h2><audio controls preload=\"metadata\" src=\"output.wav\"></audio><blockquote>“{answered}”</blockquote><dl><dt>What you see</dt><dd>The addressed response and retained synthesized WAV.</dd><dt>What happened</dt><dd>The same play routed recognized text to the House, then the admitted Piper realization synthesized its answer.</dd><dt>What Conduit established</dt><dd>Recognition, response, and synthesis completed with exact provider identities in one correlated receipt.</dd><dt>Concepts in view</dt><dd>Form · Host implementation · Plan · Play</dd><dt>Evidence</dt><dd><a href=\"response.json\">Response receipt</a> · <a href=\"receipt.json\">same-Play provider receipt</a> · <a href=\"output.wav\">output WAV</a></dd></dl></article></section><div class=\"proof-grid\"><section><h2>What this proves</h2><p>One recorded input traversed the admitted hosted recognition, addressed-response, and speech-synthesis path with correlated outputs.</p></section><section><h2>What it does not prove</h2><p>This is not a live microphone, browser-audio, speaker-playback, physical-truth, or human-audibility claim.</p></section></div><h2>Reproduce</h2><pre class=\"reproduce\"><code>cargo xtask host journey-hears-speaks-local --locked \\\n  --profile /absolute/path/hears-speaks-provider.json \\\n  --output target/journeys/hears-speaks</code></pre><details><summary>All evidence and exact downloads</summary><p>Accepted commit: <code>{}</code></p><ul><li><a href=\"input.pcm\">Admitted raw PCM</a></li><li><a href=\"input.wav\">Listen-ready input WAV</a></li><li><a href=\"recognition.json\">Recognition receipt</a></li><li><a href=\"response.json\">Response receipt</a></li><li><a href=\"receipt.json\">Same-Play and provider provenance receipt</a></li><li><a href=\"output.wav\">Synthesized output WAV</a></li><li><a href=\"manifest.json\">Digest-bound manifest</a></li></ul></details>",
        evidence.commit
    );
    write_html(
        &destination.join("index.html"),
        "Conduit Hears and Speaks",
        &body,
    )
}
