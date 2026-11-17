//! Bound the actual file read as well as its descriptive metadata.
use std::{fs, io::Read, path::Path};

pub(super) fn read(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect {}: {error}", path.display()))?;
    read_inspected(path, maximum, &metadata)
}

fn read_inspected(path: &Path, maximum: u64, metadata: &fs::Metadata) -> Result<Vec<u8>, String> {
    let invalid = || format!("{} violates its finite file bound", path.display());
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(invalid());
    }
    // The path may have changed since inspection. Read at most one byte beyond
    // the limit so growth is refused rather than accepted as a truncated file.
    // Saturation preserves u64::MAX without an overflowing sentinel addition.
    let bytes = fs::File::open(path)
        .and_then(|file| read_limited(file, maximum))
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() as u64 > maximum {
        return Err(invalid());
    }
    Ok(bytes)
}

fn read_limited(reader: impl Read, maximum: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "bounded_read_tests.rs"]
mod tests;
