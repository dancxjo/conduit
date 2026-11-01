//! Short physical custody for installation read/modify/publication transactions.
//! Never retained across Play, transport waits, or service lifetime.
use std::{
    fs::{File, OpenOptions},
    path::Path,
};
pub(crate) struct InstallationLock(File);
impl InstallationLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, String> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(root.join("installation.lock"))
            .map_err(|_| "installation lock unavailable")?;
        let meta = file
            .metadata()
            .map_err(|_| "installation lock metadata unavailable")?;
        if !meta.is_file() {
            return Err("installation lock must be a regular file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o777 != 0o600 {
                return Err("installation lock must be private".into());
            }
        }
        file.try_lock()
            .map_err(|_| "installation transaction already active")?;
        Ok(Self(file))
    }
}
impl Drop for InstallationLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
