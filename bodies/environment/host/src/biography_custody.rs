//! Keep compacted biography segments before acknowledging the ordinary Body
//! continuation boundary. Files are immutable; a conflicting copy refuses.
use crate::{checked, Result};
use conduit_body::{BodyBiographyArchiveSegment, BodyLifecycleSession};
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path};
#[cfg(test)]
#[path = "biography_custody_tests.rs"]
mod tests;
pub(super) fn retain(body: &mut BodyLifecycleSession, directory: &Path) -> Result<()> {
    let Some(head) = body.pending_archives().last().map(|s| s.digest) else {
        return Ok(());
    };
    checked(
        body.pending_archives()
            .last()
            .expect("checked head")
            .validate_as_head_of(body.evidence()),
    )?;
    std::fs::create_dir_all(directory)?;
    if let Some(parent) = directory.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    for segment in body.pending_archives() {
        checked(segment.validate())?;
        let name: String = segment.digest.iter().map(|b| format!("{b:02x}")).collect();
        let path = directory.join(format!("{name}.json"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut file) => {
                serde_json::to_writer(&mut file, segment)?;
                file.flush()?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.into()),
        }
        let stored: BodyBiographyArchiveSegment = serde_json::from_slice(&std::fs::read(&path)?)?;
        checked(stored.validate())?;
        if stored != *segment {
            return Err("biography archive conflicts with required custody".into());
        }
    }
    std::fs::File::open(directory)?.sync_all()?;
    checked(body.acknowledge_archives(head))?;
    Ok(())
}
