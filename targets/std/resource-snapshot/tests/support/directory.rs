use conduit_linux_snapshot_store::SnapshotDirectory;
use std::{
    fs::{self, File},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
pub(crate) struct Root {
    pub(crate) path: PathBuf,
}
impl Root {
    pub(crate) fn prepare() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-snapshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self { path }
    }
    pub(crate) fn directory(&self) -> SnapshotDirectory {
        SnapshotDirectory::prepare(File::open(&self.path).unwrap()).unwrap()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
