//! Immutable checked Source custody; membership remains in the ordinary biography.
use crate::plot_source::CanonicalSource;
use conduit_body::ResidentPlot;
use conduit_core::SourceDocumentId;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const MAXIMUM_BYTES: u64 = super::super::MAXIMUM_SOURCE;
const MAXIMUM_FILES: usize = 2 * conduit_body::MAX_BODY_PLOTS;

fn identity(
    source: &CanonicalSource,
    resident: &ResidentPlot,
) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    source.expand_identity_for_authoring(&resident.source_document_id, &resident.checked_plot_id)
}
fn path(root: &Path, source: &SourceDocumentId) -> PathBuf {
    root.join("body/sources").join(format!(
        "{:x}.conduit",
        Sha256::digest(source.as_str().as_bytes())
    ))
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| format!("retained Source unavailable: {e}"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAXIMUM_BYTES
    {
        return Err("retained Source violates its file bound".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAXIMUM_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAXIMUM_BYTES {
        return Err("retained Source grew beyond bound".into());
    }
    Ok(bytes)
}
/// Preparation under the installed owner's exclusive lock. Source is published
/// before biography admission; a crash can leave inert unreferenced bytes. The
/// cache is not an ingress for hostile filesystem writers or execution authority.
pub(super) fn retain(root: &Path, resident: &ResidentPlot, text: &str) -> Result<(), String> {
    if text.is_empty() || text.len() as u64 > MAXIMUM_BYTES {
        return Err("Source storage bound exceeded".into());
    }
    identity(&crate::plot_source::parse(text)?, resident)?;
    let body = root.join("body");
    let meta = fs::symlink_metadata(&body).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Body source parent is not an owned directory".into());
    }
    let directory = body.join("sources");
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("retained Sources directory is invalid".into());
    }
    crate::durable_host::restrict_directory(&directory)?;
    let destination = path(root, &resident.source_document_id);
    if fs::symlink_metadata(&destination).is_ok() {
        if read(&destination)? != text.as_bytes() {
            return Err("immutable retained Source differs".into());
        }
        return Ok(());
    }
    if fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .take(MAXIMUM_FILES + 1)
        .count()
        >= MAXIMUM_FILES
    {
        return Err("retained Source file capacity exhausted".into());
    }
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let temporary = directory.join(format!(".pending-{:x}", Sha256::digest(nonce)));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    // Atomic no-replace publication. A partial write never occupies the final
    // identity path. Any failed temporary remains inert for owner recovery.
    fs::hard_link(&temporary, &destination).map_err(|e| e.to_string())?;
    fs::remove_file(&temporary).map_err(|e| e.to_string())?;
    File::open(&directory)
        .and_then(|d| d.sync_all())
        .map_err(|e| e.to_string())?;
    let observed = read(&destination)?;
    if observed != text.as_bytes() {
        return Err("retained Source independent readback differs".into());
    }
    identity(
        &crate::plot_source::parse(std::str::from_utf8(&observed).map_err(|e| e.to_string())?)?,
        resident,
    )?;
    Ok(())
}
pub(super) fn load(
    root: &Path,
    resident: &ResidentPlot,
    primary: Option<&SourceDocumentId>,
) -> Result<(CanonicalSource, conduit_plot::ExpandedAuthoringPlot), String> {
    let selected = if primary == Some(&resident.source_document_id) {
        root.join("body/source.conduit")
    } else {
        path(root, &resident.source_document_id)
    };
    for directory in [
        root.join("body"),
        selected
            .parent()
            .ok_or("retained Source parent absent")?
            .to_owned(),
    ] {
        let metadata = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("retained Source parent is not a direct directory".into());
        }
    }
    let bytes = read(&selected)?;
    let source =
        crate::plot_source::parse(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)?;
    let checked = identity(&source, resident)?;
    Ok((source, checked))
}
