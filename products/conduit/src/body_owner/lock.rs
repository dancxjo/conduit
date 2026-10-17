//! Cooperative ownership across the installed Host service and foreground owner.
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(crate) struct OwnershipLock {
    file: File,
}

impl Drop for OwnershipLock {
    fn drop(&mut self) {
        // A fork may retain the same open file description until exec. Closing
        // only our descriptor would extend ownership into that unrelated child.
        let _ = self.file.unlock();
    }
}

pub(crate) fn acquire(root: &Path) -> Result<OwnershipLock, String> {
    named(root, "host-owner.lock")
}
pub(crate) fn body(root: &Path) -> Result<OwnershipLock, String> {
    named(root, "body-owner.lock")
}
fn named(root: &Path, name: &str) -> Result<OwnershipLock, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(name))
        .map_err(|e| format!("open Host ownership lock: {e}"))?;
    file.try_lock()
        .map_err(|e| format!("installed Host is already owned or cannot be locked: {e}"))?;
    Ok(OwnershipLock { file })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn releasing_owner_does_not_wait_for_an_inherited_descriptor_to_close() {
        let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
            "conduit-owner-lock-test",
            "inherited-descriptor",
        ));
        std::fs::create_dir_all(&root).unwrap();
        let owner = body(&root).unwrap();
        // Duplication preserves the open file description, as Unix fork does.
        let inherited = owner.file.try_clone().unwrap();
        assert!(body(&root).is_err());
        drop(owner);
        let successor = body(&root).expect("released ownership admits its successor");
        assert!(body(&root).is_err());
        drop(successor);
        drop(inherited);
        std::fs::remove_dir_all(root).unwrap();
    }
}
