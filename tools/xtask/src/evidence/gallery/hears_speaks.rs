//! Static pages for one verified hosted audio journey.

use std::path::Path;

use super::{copy_file, required_output, write_html};
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
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<h1>Conduit Hears and Speaks</h1>\n<p>One fixed recorded clip crossed Whisper, addressed House generation, Piper synthesis, PCM conversion, and bounded WAV retention in one completed Plan/Play.</p>\n<h2>Question Conduit heard</h2>\n<audio controls preload=\"metadata\" src=\"input.wav\"></audio>\n<p><a href=\"input.wav\">Download input WAV</a> · <a href=\"input.pcm\">raw admitted PCM</a> · <a href=\"recognition.json\">recognition receipt</a></p>\n<h2>Answer Conduit spoke</h2>\n<audio controls preload=\"metadata\" src=\"output.wav\"></audio>\n<p><a href=\"output.wav\">Download output WAV</a> · <a href=\"response.json\">response receipt</a> · <a href=\"receipt.json\">same-Play receipt</a> · <a href=\"manifest.json\">digest-bound manifest</a></p>\n<p>Exact accepted commit: <code>{}</code>. This is hosted recorded-audio evidence, not a live microphone, browser, emulator, physical device, or human-listening claim.</p>",
        evidence.commit
    );
    write_html(
        &destination.join("index.html"),
        "Conduit Hears and Speaks",
        &body,
    )
}
