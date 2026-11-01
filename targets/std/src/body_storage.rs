//! Explicit Linux directory custody. Declaration and opened custody alone
//! publish no Base readiness, resource offer, grant, or execution authority.
use conduit_linux_snapshot_store::{SnapshotDirectory, MAXIMUM_RECORD_BYTES};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyStorageBounds {
    pub maximum_snapshot_records: u32,
    pub maximum_record_bytes: u32,
    pub generation_slots: u16,
    pub publication_slots: u16,
    pub reader_leases: u16,
    pub state_slots: u16,
    pub maximum_state_bytes: u32,
    pub total_disk_bytes: u64,
    pub preparation_bytes: u64,
}
impl BodyStorageBounds {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=64).contains(&self.maximum_snapshot_records)
            || !(1..=4096).contains(&self.maximum_record_bytes)
            || self.generation_slots == 0
            || u32::from(self.generation_slots) > self.maximum_snapshot_records
            || !(1..=16).contains(&self.publication_slots)
            || !(1..=16).contains(&self.reader_leases)
            || self.state_slots != 1
            || !(1..=4096).contains(&self.maximum_state_bytes)
            || self.preparation_bytes == 0
            || self.preparation_bytes > 64 * 1024 * 1024
            || self.total_disk_bytes > 64 * 1024 * 1024
        {
            return Err("selected Body storage exceeds its finite implementation profile".into());
        }
        if self.total_disk_bytes < self.required_disk_bytes()? {
            return Err("selected Body storage lacks candidate/current disk envelope".into());
        }
        Ok(())
    }
    /// Bound all retained immutable records plus independently in-flight
    /// candidates, and current/candidate State records. This is a constraint,
    /// not an OS quota or proof of full filesystem capacity reservation.
    pub fn required_disk_bytes(&self) -> Result<u64, String> {
        let snapshots = u64::from(self.maximum_snapshot_records)
            .checked_add(u64::from(self.publication_slots))
            .and_then(|n| n.checked_mul(MAXIMUM_RECORD_BYTES as u64))
            .ok_or("snapshot disk bound overflow")?;
        let state = crate::state_value::durable_record_byte_bound(self.maximum_state_bytes)? as u64;
        snapshots
            .checked_add(
                state
                    .checked_mul(2 * u64::from(self.state_slots))
                    .ok_or("State disk bound overflow")?,
            )
            .ok_or("storage disk bound overflow".into())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyStorageRootIdentity {
    pub canonical_path: PathBuf,
    pub device: u64,
    pub inode: u64,
}
pub struct SelectedBodyStorage {
    directory: SnapshotDirectory,
    state_root: File,
    identity: BodyStorageRootIdentity,
    pool_namespace: String,
    bounds: BodyStorageBounds,
    state_path: PathBuf,
    pub(crate) generation: Option<super::body_storage_generation::SelectedBodyStorageGeneration>,
}
impl SelectedBodyStorage {
    /// Effectful trusted selection, separate from checking host.conduit.
    /// Reopening for a fresh Boot must supply the retained exact identity.
    pub fn open(
        path: &Path,
        pool_namespace: &str,
        bounds: BodyStorageBounds,
        expected: Option<&BodyStorageRootIdentity>,
    ) -> Result<Self, String> {
        bounds.validate()?;
        if !path.is_absolute()
            || path.as_os_str().as_bytes().len() > 4096
            || pool_namespace.is_empty()
            || pool_namespace.len() > 96
            || !pool_namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        {
            return Err("Body storage requires an explicit finite root and pool namespace".into());
        }
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("selected Body storage root: {e}"))?;
        if canonical.as_os_str().as_bytes().len() > 4096 {
            return Err("canonical Body storage root exceeds finite path profile".into());
        }
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(&canonical)
            .map_err(|e| format!("open Body storage root: {e}"))?;
        let metadata = root.metadata().map_err(|e| e.to_string())?;
        // SAFETY: geteuid has no preconditions and returns this process identity.
        let owner_uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.uid() != owner_uid || metadata.mode() & 0o077 != 0 {
            return Err("selected Body storage root must be an owned private directory".into());
        }
        let identity = BodyStorageRootIdentity {
            canonical_path: canonical,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        if expected.is_some_and(|expected| expected != &identity) {
            return Err("selected Body storage root identity changed".into());
        }
        let observed = std::fs::metadata(&identity.canonical_path).map_err(|e| e.to_string())?;
        if observed.dev() != identity.device || observed.ino() != identity.inode {
            return Err("Body storage root changed during selection".into());
        }
        let state_root = root.try_clone().map_err(|e| e.to_string())?;
        let state_path = PathBuf::from(format!("/proc/self/fd/{}", state_root.as_raw_fd()));
        let directory = SnapshotDirectory::prepare(root)
            .map_err(|e| format!("prepare selected Body storage directory: {e}"))?;
        let selected = Self {
            directory,
            state_root,
            identity,
            pool_namespace: pool_namespace.into(),
            bounds,
            state_path,
            generation: None,
        };
        selected.require_preparation_capacity(selected.custody_preparation_bytes())?;
        Ok(selected)
    }
    pub fn identity(&self) -> &BodyStorageRootIdentity {
        &self.identity
    }
    pub fn bounds(&self) -> &BodyStorageBounds {
        &self.bounds
    }
    pub fn pool_namespace(&self) -> &str {
        &self.pool_namespace
    }
    /// Exact currently owned struct/String/Path buffer footprint. This narrow
    /// quantity excludes the Host shared-owner bookkeeping and is not the
    /// complete operation or Body preparation envelope.
    pub fn custody_preparation_bytes(&self) -> u64 {
        (std::mem::size_of::<Self>()
            + self.identity.canonical_path.capacity()
            + self.pool_namespace.capacity()
            + self.state_path.capacity()) as u64
    }
    /// Operation preparation must add its actual bounded workspace and the
    /// original complete Body Plan admission before advertising readiness.
    pub fn require_preparation_capacity(&self, required: u64) -> Result<(), String> {
        if required > self.bounds.preparation_bytes {
            Err("selected Body storage preparation capacity is insufficient".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn root_descriptor(&self) -> &File {
        &self.state_root
    }
    pub(crate) fn snapshot_directory(&self) -> &SnapshotDirectory {
        &self.directory
    }
    /// Never serialized. The held descriptor remains live through quiescence;
    /// effects use this derived root rather than reopening an ambient path.
    pub(crate) fn state_root(&self) -> &Path {
        let _keep_alive = &self.state_root;
        &self.state_path
    }
}
impl crate::StdHost {
    pub fn attach_selected_body_storage(
        &mut self,
        selected: SelectedBodyStorage,
    ) -> Result<(), String> {
        if !self.kernel_resources.is_idle() || self.selected_body_storage.is_some() {
            return Err("Body storage selection requires a fresh idle Host".into());
        }
        // No offer, Base Ready, grant, inventory refresh or ledger replacement.
        self.selected_body_storage = Some(Arc::new(selected));
        Ok(())
    }
    pub fn selected_body_storage(&self) -> Option<&SelectedBodyStorage> {
        self.selected_body_storage.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds() -> BodyStorageBounds {
        BodyStorageBounds {
            maximum_snapshot_records: 2,
            maximum_record_bytes: 4096,
            generation_slots: 2,
            publication_slots: 1,
            reader_leases: 1,
            state_slots: 1,
            maximum_state_bytes: 1024,
            total_disk_bytes: 65536,
            preparation_bytes: 8192,
        }
    }
    #[test]
    fn descriptor_custody_survives_rename_but_fresh_boot_refuses_replacement_without_offers() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("selected");
        std::fs::create_dir(&root).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let selected = SelectedBodyStorage::open(&root, "body/storage", bounds(), None).unwrap();
        let identity = selected.identity().clone();
        let before_host = crate::StdHost::new();
        let before = before_host.advertisement().clone();
        let mut host = before_host;
        host.attach_selected_body_storage(selected).unwrap();
        assert_eq!(host.advertisement(), &before);
        let selected = host.selected_body_storage().unwrap();
        assert!(selected.custody_preparation_bytes() > 0);
        std::fs::rename(&root, parent.path().join("moved")).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let descriptor_metadata = std::fs::metadata(selected.state_root()).unwrap();
        assert_eq!(
            (descriptor_metadata.dev(), descriptor_metadata.ino()),
            (identity.device, identity.inode)
        );
        assert!(
            SelectedBodyStorage::open(&root, "body/storage", bounds(), Some(&identity))
                .err()
                .unwrap()
                .contains("identity changed")
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let _held_snapshot = selected.snapshot_directory();
    }
    #[test]
    fn worker_borrow_retains_the_same_descriptor_owner_without_reopening_root() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("selected");
        std::fs::create_dir(&root).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let selected = SelectedBodyStorage::open(&root, "body/storage", bounds(), None).unwrap();
        let identity = selected.identity().clone();
        let mut host = crate::StdHost::new();
        let advertised = host.advertisement().clone();
        host.attach_selected_body_storage(selected).unwrap();
        // Internal execution borrows the same owned descriptors. This handle
        // neither escapes as a public selection nor authorizes an operation.
        let retained = Arc::clone(host.selected_body_storage.as_ref().unwrap());
        assert!(std::ptr::eq(
            host.selected_body_storage().unwrap(),
            retained.as_ref()
        ));
        assert_eq!(host.advertisement(), &advertised);
        let lifetime = Arc::downgrade(&retained);
        std::fs::rename(&root, parent.path().join("moved")).unwrap();
        std::fs::create_dir(&root).unwrap();
        drop(host);
        let metadata = std::fs::metadata(retained.state_root()).unwrap();
        assert_eq!(
            (metadata.dev(), metadata.ino()),
            (identity.device, identity.inode)
        );
        assert_eq!(Arc::strong_count(&retained), 1);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        drop(retained);
        assert!(lifetime.upgrade().is_none());
    }
    #[test]
    fn insufficient_candidate_disk_or_preparation_refuses_without_filesystem_effect() {
        let root = tempfile::tempdir().unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut limits = bounds();
        limits.total_disk_bytes = 1;
        assert!(
            SelectedBodyStorage::open(root.path(), "body/storage", limits, None)
                .err()
                .unwrap()
                .contains("disk envelope")
        );
        let mut limits = bounds();
        limits.preparation_bytes = 1;
        assert!(
            SelectedBodyStorage::open(root.path(), "body/storage", limits, None)
                .err()
                .unwrap()
                .contains("preparation capacity")
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
