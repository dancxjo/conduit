use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::commands::conduitos::ConduitosError;

use super::ProofResult;

const MAXIMUM_RETAINED_FILES: usize = 256;
const MAXIMUM_RETAINED_BYTES: u64 = 32 * 1024 * 1024;
static NEXT_BATCH: AtomicU64 = AtomicU64::new(0);

pub(super) struct BatchTempRoot {
    path: PathBuf,
}

impl BatchTempRoot {
    pub(super) fn new() -> Result<Self, ConduitosError> {
        let sequence = NEXT_BATCH.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("conduit-x86-{}-{sequence}", std::process::id()));
        let longest_socket = path.join("product-journey/x86_64/journey-monitor.sock");
        if longest_socket.as_os_str().len() >= 108 {
            return Err(ConduitosError::refusal(
                "proof-batch-temp-path-too-long",
                format!(
                    "QMP socket path exceeds the UNIX bound: {}",
                    longest_socket.display()
                ),
            ));
        }
        fs::create_dir(&path).map_err(|error| {
            ConduitosError::refusal(
                "proof-batch-temp-unavailable",
                format!("{}: {error}", path.display()),
            )
        })?;
        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BatchTempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn print_failure_tails(results: &[ProofResult]) {
    for result in results.iter().filter(|result| result.status != "success") {
        let Ok(contents) = fs::read_to_string(&result.stderr_log) else {
            continue;
        };
        let lines: Vec<_> = contents.lines().rev().take(24).collect();
        eprintln!("--- {} stderr tail ---", result.proof.as_str());
        for line in lines.into_iter().rev() {
            eprintln!("{line}");
        }
    }
}

pub(super) fn failure_names(results: &[ProofResult]) -> Vec<&'static str> {
    results
        .iter()
        .filter(|result| result.status != "success")
        .map(|result| result.proof.as_str())
        .collect()
}

pub(super) fn retain_bounded_outputs(
    source: &Path,
    destination: &Path,
) -> Result<(), ConduitosError> {
    if !source.is_dir() {
        return Ok(());
    }
    let mut pending = vec![source.to_owned()];
    let mut files = 0usize;
    let mut bytes = 0u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(retention_error)? {
            let entry = entry.map_err(retention_error)?;
            let kind = entry.file_type().map_err(retention_error)?;
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !kind.is_file() || !is_evidence_file(&entry.path()) {
                continue;
            }
            let size = entry.metadata().map_err(retention_error)?.len();
            files += 1;
            bytes = bytes.checked_add(size).ok_or_else(retention_bound_error)?;
            if files > MAXIMUM_RETAINED_FILES || bytes > MAXIMUM_RETAINED_BYTES {
                return Err(retention_bound_error());
            }
            let relative = entry
                .path()
                .strip_prefix(source)
                .map_err(|error| {
                    ConduitosError::refusal("proof-batch-retention-failed", error.to_string())
                })?
                .to_owned();
            let retained = destination.join(relative);
            if let Some(parent) = retained.parent() {
                fs::create_dir_all(parent).map_err(retention_error)?;
            }
            fs::copy(entry.path(), retained).map_err(retention_error)?;
        }
    }
    Ok(())
}

fn is_evidence_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("json" | "log" | "png")
    )
}

fn retention_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("proof-batch-retention-failed", error.to_string())
}

fn retention_bound_error() -> ConduitosError {
    ConduitosError::refusal(
        "proof-batch-retention-bound",
        format!(
            "retained evidence exceeds {MAXIMUM_RETAINED_FILES} files or {MAXIMUM_RETAINED_BYTES} bytes"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_qmp_namespace_fits_the_unix_socket_bound() {
        let root = BatchTempRoot::new().unwrap();
        let longest = root
            .path()
            .join("product-journey/x86_64/journey-monitor.sock");
        assert!(longest.as_os_str().len() < 108, "{}", longest.display());
    }

    #[test]
    fn retention_keeps_evidence_and_omits_machine_payloads() {
        let root = BatchTempRoot::new().unwrap();
        let source = root.path().join("source/x86_64");
        let destination = root.path().join("retained");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("proof.json"), b"{}").unwrap();
        fs::write(source.join("serial.log"), b"sign").unwrap();
        fs::write(source.join("machine.iso"), b"large payload").unwrap();

        retain_bounded_outputs(&source, &destination).unwrap();

        assert_eq!(fs::read(destination.join("proof.json")).unwrap(), b"{}");
        assert_eq!(fs::read(destination.join("serial.log")).unwrap(), b"sign");
        assert!(!destination.join("machine.iso").exists());
    }
}
