//! A private short-lived QMP endpoint independent of the checkout path length.
use super::ConduitosError;
use std::{
    ffi::CStr,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) struct MonitorSocket {
    directory: PathBuf,
    path: PathBuf,
}

impl MonitorSocket {
    pub(super) fn new() -> Result<Self, ConduitosError> {
        // These emulator lanes run on Unix. mkdtemp atomically creates a
        // private 0700 directory and never reuses another run's endpoint.
        let mut template = b"/tmp/conduit-qmp-XXXXXX\0".to_vec();
        // SAFETY: template is writable, NUL terminated, and retains its storage
        // throughout mkdtemp and the subsequent path copy.
        let directory = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
        if directory.is_null() {
            return Err(ConduitosError::refusal(
                "qemu-monitor-directory-unavailable",
                std::io::Error::last_os_error().to_string(),
            ));
        }
        // SAFETY: successful mkdtemp returns the original NUL-terminated buffer.
        let directory = PathBuf::from(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(directory) }.to_bytes(),
        ));
        let path = directory.join("monitor.sock");
        Ok(Self { directory, path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MonitorSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    #[test]
    fn independent_private_endpoints_fit_unix_limit_and_remove_only_owned_files() {
        let first = MonitorSocket::new().unwrap();
        let second = MonitorSocket::new().unwrap();
        assert_ne!(first.path(), second.path());
        assert!(first.path().as_os_str().as_bytes().len() < 104);
        assert_eq!(
            fs::metadata(&first.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let path = first.path().to_owned();
        let directory = first.directory.clone();
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        drop(first);
        assert!(!path.exists());
        assert!(!directory.exists());
        assert!(second.directory.is_dir());
    }
}
