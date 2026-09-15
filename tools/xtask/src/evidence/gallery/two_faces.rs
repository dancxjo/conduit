//! Static exhibit for one Presentation through two distinct renderers.

use std::path::Path;

use super::{copy_file, required_output, write_html};
use crate::evidence::VerifiedEvidence;

const FILES: &[(&str, &str)] = &[
    ("two-faces.native-frame", "native.png"),
    ("two-faces.native-receipt", "native.json"),
    ("two-faces.browser-frame", "browser.png"),
    ("two-faces.browser-receipt", "browser.json"),
];

pub(super) fn write_two_faces_commit(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root
            .join("commits")
            .join(&evidence.commit)
            .join("one-form-two-faces"),
        evidence_root,
        evidence,
        "../../../index.html",
    )
}

pub(super) fn write_two_faces_current(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root.join("current/one-form-two-faces"),
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
            .map_err(|error| format!("cannot replace Two Faces exhibit: {error}"))?;
    }
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create Two Faces exhibit: {error}"))?;
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
    let presentation = required_output(evidence, "two-faces.native-frame")?
        .provenance
        .presentation_id
        .as_deref()
        .ok_or("Two Faces native frame lacks Presentation identity")?;
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<p class=\"eyebrow\">One meaning, two manifestations</p><h1>One Form, Two Faces</h1><p class=\"lede\">The recognizable Morse Network Form below became one Presentation. Two Hosts then manifested that same semantic identity in their own native language.</p><div class=\"identity-flow\"><strong>one authored Morse Network Form</strong><span>↓</span><code>{presentation}</code><span>↙︎ &nbsp; ↘︎</span><span>native renderer &nbsp; browser renderer</span></div>\n<div class=\"comparison\"><figure><img src=\"native.png\" alt=\"Morse Network Form manifested by the native software renderer\"><figcaption>Native manifestation</figcaption></figure><figure><img src=\"browser.png\" alt=\"The same Morse Network Presentation manifested in pinned Chromium\"><figcaption>Browser manifestation</figcaption></figure></div><p class=\"boundary\"><strong>Shared semantic identity, distinct renderings.</strong> Pixel equality, physical display output, and human perception are not claimed.</p><details><summary>Evidence and exact identities</summary><p>Accepted commit: <code>{}</code></p><ul><li><a href=\"native.json\">Native renderer receipt</a></li><li><a href=\"browser.json\">Pinned Chromium receipt</a></li><li><a href=\"manifest.json\">Digest-bound manifest</a></li></ul></details>",
        evidence.commit
    );
    write_html(
        destination.join("index.html").as_path(),
        "One Form, Two Faces",
        &body,
    )
}
