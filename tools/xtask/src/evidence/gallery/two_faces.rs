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
    let presentation = required_output(evidence, "two-faces.native-frame")?
        .provenance
        .presentation_id
        .as_deref()
        .ok_or("Two Faces native frame lacks Presentation identity")?;
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<h1>One Form, Two Faces</h1>\n<p>The same portable Presentation identity was manifested by distinct native and pinned-browser renderer Plans. Pixel equality is neither expected nor claimed.</p>\n<div class=\"comparison\"><figure><img src=\"native.png\" alt=\"Native software-rendered Patchbay manifestation\"><figcaption>Native software renderer · <a href=\"native.json\">receipt</a></figcaption></figure><figure><img src=\"browser.png\" alt=\"Pinned Chromium Patchbay manifestation\"><figcaption>Pinned Chromium DOM/SVG renderer · <a href=\"browser.json\">receipt</a></figcaption></figure></div>\n<p>Shared Presentation: <code>{presentation}</code>.</p>\n<p>Exact accepted commit: <code>{}</code>. This is retained native software-render and live pinned-browser evidence, not visual equality, physical display, or human-perception proof.</p>",
        evidence.commit
    );
    write_html(
        destination.join("index.html").as_path(),
        "One Form, Two Faces",
        &body,
    )
}
