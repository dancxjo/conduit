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
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<p class=\"eyebrow\">A question became an answer</p><h1>Conduit Hears and Speaks</h1>\n<p class=\"lede\">Conduit listened to one bounded recording, recognized who was being addressed, asked the House, and made a new voice recording in reply.</p>\n<section class=\"media-card\"><p class=\"step\">1 · The question</p><h2>What Conduit heard</h2><audio controls preload=\"metadata\" src=\"input.wav\"></audio><blockquote>“{recognized}”</blockquote><p><a href=\"input.wav\" download>Download the question</a></p></section>\n<div class=\"story-arrow\" aria-hidden=\"true\">recording → Whisper → addressed House → response → Piper → WAV</div>\n<section class=\"media-card\"><p class=\"step\">2 · The answer</p><h2>What Conduit said</h2><audio controls preload=\"metadata\" src=\"output.wav\"></audio><blockquote>“{answered}”</blockquote><p><a href=\"output.wav\" download>Download the answer</a></p></section>\n<p class=\"boundary\"><strong>Hosted recorded-audio proof.</strong> This establishes provider execution and one correlated Plan/Play—not a live microphone, browser audio execution, speaker playback, physical truth, or human audibility.</p>\n<details><summary>Evidence and exact downloads</summary><p>Accepted commit: <code>{}</code></p><ul><li><a href=\"input.pcm\">Admitted raw PCM</a></li><li><a href=\"recognition.json\">Recognition receipt</a></li><li><a href=\"response.json\">Response receipt</a></li><li><a href=\"receipt.json\">Same-Play and provider provenance receipt</a></li><li><a href=\"manifest.json\">Digest-bound manifest</a></li></ul></details>",
        evidence.commit
    );
    write_html(
        &destination.join("index.html"),
        "Conduit Hears and Speaks",
        &body,
    )
}
