//! Read-only finite census under the selected descriptor. Observed free blocks
//! are not an exclusive reservation; late ENOSPC remains an effect failure.
use super::*;
use conduit_linux_snapshot_store::MAXIMUM_RECORD_BYTES;
use std::fs::Metadata;
pub(super) struct StorageCensus {
    pub snapshots: u32,
    pub states: u32,
    pub allocated: u64,
    pub logical: u64,
    pub available: u64,
    pub workspace_bytes: u64,
}
#[derive(Default)]
struct Usage {
    logical: u64,
    allocated: u64,
    inodes: Vec<(u64, u64)>,
}
impl Usage {
    fn add(&mut self, m: &Metadata) -> Result<(), String> {
        if !self.inodes.contains(&(m.dev(), m.ino())) {
            if self.inodes.len() == self.inodes.capacity() {
                return Err("storage inode census capacity exceeded".into());
            }
            self.inodes.push((m.dev(), m.ino()));
            self.logical = self
                .logical
                .checked_add(m.len())
                .ok_or("storage logical size overflow")?;
            self.allocated = self
                .allocated
                .checked_add(
                    m.blocks()
                        .checked_mul(512)
                        .ok_or("allocated block overflow")?,
                )
                .ok_or("storage allocated size overflow")?;
        }
        Ok(())
    }
}
fn hex(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn file(path: &Path, max: u64, usage: &mut Usage) -> Result<Metadata, String> {
    let fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| format!("storage entry: {e}"))?;
    let metadata = fd.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > max || metadata.uid() != unsafe { libc::geteuid() } {
        return Err("storage entry exceeds selected finite regular-file profile".into());
    }
    usage.add(&metadata)?;
    Ok(metadata)
}
impl StorageCensus {
    pub fn inspect(
        storage: &SelectedBodyStorage,
        expected: Option<&str>,
        publish: bool,
        expected_state: Option<&str>,
    ) -> Result<Self, String> {
        let with_state = expected_state.is_some();
        let root = storage.state_root();
        let directory = storage
            .root_descriptor()
            .try_clone()
            .map_err(|e| e.to_string())?;
        let before = directory.metadata().map_err(|e| e.to_string())?;
        let bounds = storage.bounds();
        if (publish || with_state) && before.mode() & 0o700 != 0o700 {
            return Err("storage root is not owner-readable/writable/searchable".into());
        }
        let max_files = bounds.maximum_snapshot_records as usize
            + bounds.publication_slots as usize
            + 3 * bounds.state_slots as usize;
        let mut usage = Usage {
            inodes: Vec::with_capacity(max_files),
            ..Usage::default()
        };
        let mut snapshots = 0u32;
        let mut states = 0u32;
        let mut found = false;
        let entry_limit = u64::from(bounds.maximum_snapshot_records)
            + u64::from(bounds.publication_slots)
            + u64::from(bounds.state_slots);
        let mut entries = 0u64;
        // /proc/self/fd is derived only from the still-held descriptor, never
        // from the original ambient canonical path.
        for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
            entries += 1;
            if entries > entry_limit {
                return Err("selected storage entry capacity exhausted".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("noncanonical storage entry")?;
            if name.starts_with(".pending-") {
                return Err("unreconciled snapshot candidate prevents new generation".into());
            }
            if !hex(name) {
                return Err("unknown storage entry prevents bounded generation selection".into());
            }
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if metadata.is_file() {
                let opened = file(&entry.path(), MAXIMUM_RECORD_BYTES as u64, &mut usage)?;
                if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                    return Err("storage entry changed during census".into());
                }
                snapshots += 1;
                if expected == Some(name) {
                    found = true;
                }
                if snapshots > bounds.maximum_snapshot_records {
                    return Err("snapshot retained slot capacity exceeded".into());
                }
            } else if metadata.is_dir() {
                if expected_state.is_some_and(|expected| expected != name) {
                    return Err("occupied State slot belongs to another exact binding".into());
                }
                states += 1;
                if states > u32::from(bounds.state_slots) {
                    return Err("State retained slot capacity exceeded".into());
                }
                let fd = open_directory(&entry.path())?;
                let actual = fd.metadata().map_err(|e| e.to_string())?;
                if actual.dev() != metadata.dev()
                    || actual.ino() != metadata.ino()
                    || actual.uid() != unsafe { libc::geteuid() }
                {
                    return Err("State directory changed during census".into());
                }
                usage.add(&actual)?;
                let nested = std::path::PathBuf::from(format!("/proc/self/fd/{}", fd.as_raw_fd()));
                let mut children = 0;
                for child in std::fs::read_dir(nested).map_err(|e| e.to_string())? {
                    children += 1;
                    if children > 2 {
                        return Err("State file capacity exceeded".into());
                    }
                    let child = child.map_err(|e| e.to_string())?;
                    if child.file_name() != "current" {
                        return Err(
                            "unreconciled or unknown State candidate prevents selection".into()
                        );
                    }
                    file(
                        &child.path(),
                        crate::state_value::durable_record_byte_bound(bounds.maximum_state_bytes)?
                            as u64,
                        &mut usage,
                    )?;
                }
            } else {
                return Err("nonregular storage entry prevents generation selection".into());
            }
        }
        if expected.is_some() && ((publish && found) || (!publish && !found)) {
            return Err("exact snapshot publication/read residence is unavailable".into());
        }
        if expected.is_some() && publish && snapshots >= bounds.maximum_snapshot_records {
            return Err("no retained immutable snapshot slot remains".into());
        }
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        if unsafe { libc::fstatvfs(directory.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(format!(
                "storage filesystem capacity: {}",
                std::io::Error::last_os_error()
            ));
        }
        let stat = unsafe { stat.assume_init() };
        let block = stat.f_frsize as u64;
        if block == 0 || stat.f_flag & libc::ST_RDONLY as libc::c_ulong != 0 {
            return Err("selected storage filesystem is not writable".into());
        }
        let available = (stat.f_bavail as u64)
            .checked_mul(block)
            .ok_or("filesystem available size overflow")?;
        let round = |n: u64| {
            n.checked_add(block - 1)
                .and_then(|v| v.checked_div(block))
                .and_then(|v| v.checked_mul(block))
                .ok_or("filesystem block rounding overflow")
        };
        let snapshot_growth = if expected.is_some() && publish {
            round(MAXIMUM_RECORD_BYTES as u64)?
        } else {
            0
        };
        let state_growth = if with_state {
            2 * round(
                crate::state_value::durable_record_byte_bound(bounds.maximum_state_bytes)? as u64,
            )? + if states == 0 { block } else { 0 }
        } else {
            0
        };
        let growth = snapshot_growth
            .checked_add(state_growth)
            .ok_or("storage growth overflow")?;
        let declared_used = usage.logical.max(usage.allocated);
        if declared_used
            .checked_add(growth)
            .ok_or("storage capacity overflow")?
            > bounds.total_disk_bytes
            || growth > available
            || stat.f_favail
                < (u64::from(expected.is_some() && publish)
                    + if with_state {
                        2 + u64::from(states == 0)
                    } else {
                        0
                    }) as _
        {
            return Err("selected filesystem or declared disk capacity is insufficient".into());
        }
        let after = directory.metadata().map_err(|e| e.to_string())?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err("storage directory changed during census".into());
        }
        let workspace_bytes = (usage.inodes.capacity() * std::mem::size_of::<(u64, u64)>()
            + std::mem::size_of::<Self>()
            + std::mem::size_of::<Usage>()
            + std::mem::size_of::<libc::statvfs>()) as u64;
        Ok(Self {
            snapshots,
            states,
            allocated: usage.allocated,
            logical: usage.logical,
            available,
            workspace_bytes,
        })
    }
}
