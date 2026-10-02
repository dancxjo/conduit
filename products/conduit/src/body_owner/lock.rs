//! Cooperative ownership across the installed Host service and foreground owner.
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(crate) fn acquire(root: &Path) -> Result<File, String> {
    named(root, "host-owner.lock")
}
pub(crate) fn body(root: &Path) -> Result<File, String> {
    named(root, "body-owner.lock")
}
fn named(root: &Path, name: &str) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(name))
        .map_err(|e| format!("open Host ownership lock: {e}"))?;
    file.try_lock()
        .map_err(|e| format!("installed Host is already owned or cannot be locked: {e}"))?;
    Ok(file)
}
