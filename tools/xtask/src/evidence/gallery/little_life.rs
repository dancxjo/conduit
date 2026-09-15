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
    let frames = [("t000.png", "t = 0"), ("t001.png", "t = 1"), ("t008.png", "t = 8"), ("t032.png", "t = 32")]
        .into_iter()
        .map(|(path, label)| format!("<figure><img src=\"{path}\" alt=\"Orbium scalar field at {label}\"><figcaption>{label}</figcaption></figure>"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<h1>Little Life</h1>\n<p>One deterministic Orbium seed entered an ordinary 32-step fixed-point Lenia Plan/Play. These four retained checkpoints show the bounded evolution.</p>\n<div class=\"comparison\">{frames}</div>\n<p><a href=\"presentation.txt\">Complete scalar-field terminal presentation</a> · <a href=\"execution.json\">Plan/Play execution report</a></p>\n<p>Exact accepted commit: <code>{}</code>. Generation zero is deterministic semantic seed lowering; later images derive from the installed std terminal scalar-field presentation. This is not a native graphical renderer, physical display, or human-perception claim.</p>",
        evidence.commit
    );
    write_html(&destination.join("index.html"), "Little Life", &body)
}
