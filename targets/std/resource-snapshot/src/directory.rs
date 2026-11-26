use std::{
    ffi::CStr,
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::OpenOptionsExt,
    },
};

/// Actual retained directory descriptor, supplied by trusted Root preparation.
/// Paths are resolved relative to this descriptor even if its old path moves.
/// This is not a Host offer or proof of a planned Resource pool's identity.
pub struct SnapshotDirectory {
    directory: File,
}
impl SnapshotDirectory {
    /// Open an existing operator-selected directory without following its final
    /// symlink or accepting a file/device instead. Descriptor custody grants no
    /// Resource reference authority and reserves no disk capacity. Parent path
    /// components retain the caller's ordinary path-resolution responsibility.
    pub fn open_selected(path: &std::path::Path) -> io::Result<Self> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        Self::prepare(directory)
    }
    pub fn prepare(directory: File) -> io::Result<Self> {
        if !directory.metadata()?.is_dir() {
            return Err(io::Error::from_raw_os_error(libc::ENOTDIR));
        }
        // Reject filesystems/descriptors that cannot sync a directory before
        // any Play. This creates no file and reserves no filesystem capacity.
        directory.sync_all()?;
        Ok(Self { directory })
    }
    pub(crate) fn open(&self, name: &CStr, flags: i32) -> io::Result<File> {
        // SAFETY: name is terminated, the retained descriptor stays open, and
        // the returned descriptor is owned exactly once on success.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    pub(crate) fn link(&self, candidate: &CStr, published: &CStr) -> io::Result<()> {
        // SAFETY: both names are terminated and both directory descriptors are
        // retained. linkat without replacement fails when published exists.
        let result = unsafe {
            libc::linkat(
                self.directory.as_raw_fd(),
                candidate.as_ptr(),
                self.directory.as_raw_fd(),
                published.as_ptr(),
                0,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub(crate) fn unlink(&self, name: &CStr) -> io::Result<()> {
        // SAFETY: name is terminated and names only our exact candidate file.
        let result = unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub(crate) fn sync(&self) -> io::Result<()> {
        self.directory.sync_all()
    }
}

pub(crate) struct Name {
    bytes: [u8; 80],
    length: usize,
}
impl Name {
    pub(crate) fn prepare(key: &str, candidate: bool) -> Option<Self> {
        let digest = key.strip_prefix("resource/")?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let prefix: &[u8] = if candidate { b".pending-" } else { b"" };
        let mut result = Self {
            bytes: [0; 80],
            length: prefix.len() + digest.len(),
        };
        result.bytes[..prefix.len()].copy_from_slice(prefix);
        result.bytes[prefix.len()..result.length].copy_from_slice(digest.as_bytes());
        Some(result)
    }
    pub(crate) fn cstr(&self) -> &CStr {
        CStr::from_bytes_with_nul(&self.bytes[..=self.length]).expect("prepared hex name")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Read, path::PathBuf};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "conduit-selected-snapshot-directory-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn selected_directory_refuses_missing_file_and_final_symlink() {
        let root = Directory::new();
        let missing = root.0.join("missing");
        assert!(SnapshotDirectory::open_selected(&missing).is_err());
        let file = root.0.join("file");
        fs::write(&file, b"not a directory").unwrap();
        assert!(SnapshotDirectory::open_selected(&file).is_err());
        let actual = root.0.join("actual");
        fs::create_dir(&actual).unwrap();
        let link = root.0.join("linked");
        std::os::unix::fs::symlink(&actual, &link).unwrap();
        assert!(SnapshotDirectory::open_selected(&link).is_err());
        assert!(SnapshotDirectory::open_selected(&actual).is_ok());
    }
    #[test]
    fn selected_descriptor_keeps_original_directory_after_path_replacement() {
        let root = Directory::new();
        let selected = root.0.join("selected");
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("record"), b"original").unwrap();
        let retained = SnapshotDirectory::open_selected(&selected).unwrap();
        fs::rename(&selected, root.0.join("moved-original")).unwrap();
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("record"), b"replacement").unwrap();
        let name = c"record";
        let mut bytes = String::new();
        retained
            .open(name, libc::O_RDONLY)
            .unwrap()
            .read_to_string(&mut bytes)
            .unwrap();
        assert_eq!(bytes, "original");
        assert_eq!(fs::read(selected.join("record")).unwrap(), b"replacement");
        // SAFETY: F_GETFD only inspects the live descriptor retained above.
        let flags = unsafe { libc::fcntl(retained.directory.as_raw_fd(), libc::F_GETFD) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }
}
