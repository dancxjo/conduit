//! Lightweight entrance; provider and decoder dependencies remain in std-host.
use std::process::Command;
#[path = "../../../targets/std/examples/ambient_photo_prepare/options.rs"]
mod options;

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("ambient photos require the Unix no-follow cache profile".into());
    }
    let checked = options::parse(arguments)?;
    // The same parser belongs to the preparation harness; no provider machinery
    // enters the dependency-light dispatcher. Read all fields to keep this
    // shared parser usable in both binaries without suppressing dead-code checks.
    let _ = (
        checked.date,
        checked.output,
        checked.source,
        checked.manifest,
        checked.checksum,
    );
    let status = Command::new("cargo")
        .current_dir(crate::workspace::workspace_root()?)
        .args([
            "run",
            "--locked",
            "--package",
            "conduit-std-host",
            "--example",
            "ambient_photo_prepare",
            "--",
        ])
        .args(arguments)
        .status()
        .map_err(|e| format!("launch admitted photo preparation: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ambient-photo preparation failed: {status}"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn source_profiles_are_explicit_and_never_silent_fallbacks() {
        assert!(options::parse(&args(&[
            "--date",
            "2024-01-05",
            "--output",
            "target/photos"
        ]))
        .is_ok());
        assert!(options::parse(&args(&[
            "--date",
            "2024-01-05",
            "--output",
            "target/photos",
            "--source",
            "candidate-manifest"
        ]))
        .is_err());
        let checksum = "a".repeat(64);
        let manifest = args(&[
            "--date",
            "2024-01-05",
            "--output",
            "target/photos",
            "--source",
            "candidate-manifest",
            "--manifest",
            "target/candidates.json",
            "--manifest-sha256",
            &checksum,
        ]);
        assert_eq!(
            options::parse(&manifest).unwrap().source,
            "candidate-manifest"
        );
    }
}
