use std::{
    ffi::CStr,
    fs::File,
    io,
    os::fd::{AsRawFd, FromRawFd},
};

/// Actual retained directory descriptor, supplied by trusted Root preparation.
/// Paths are resolved relative to this descriptor even if its old path moves.
/// This is not a Host offer or proof of a planned Resource pool's identity.
pub struct SnapshotDirectory {
    directory: File,
}
impl SnapshotDirectory {
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
