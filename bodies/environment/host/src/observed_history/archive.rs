//! Prepared immutable signed evidence, not a recorder or admission authority.
//! Directory traversal, verification and allocation occur before Host selection.
use super::*;
use conduit_environment_runtime::history_exchange::{HistoryProvider, HistoryProviderRefusal};

pub(crate) struct Limits {
    pub directory_entries: usize,
    pub batches: usize,
    pub bytes: usize,
}
struct Entry {
    after: u32,
    bytes: Box<[u8]>,
}
pub(crate) struct Archive {
    entries: Box<[Entry]>,
    revoked: bool,
}
impl Collector {
    /// The caller supplies an authenticated scope and finite preparation limits.
    /// These limits do not grant storage residence, a Host Call or a planned Line.
    pub(crate) fn prepare_archive(&self, directory: &Path, limits: Limits) -> Result<Archive> {
        if limits.directory_entries == 0
            || limits.batches == 0
            || limits.batches > 960
            || limits.batches > limits.directory_entries
            || limits.bytes == 0
            || limits.bytes > limits.batches.saturating_mul(2048)
        {
            return Err("history archive limits".into());
        }
        let mut paths = Vec::new();
        for (index, entry) in std::fs::read_dir(directory)?.enumerate() {
            if index >= limits.directory_entries {
                return Err("history archive directory capacity".into());
            }
            let path = entry?.path();
            if path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("observed-history-") && name.ends_with(".json")
            }) {
                if paths.len() == limits.batches {
                    return Err("history archive batch capacity".into());
                }
                paths.push(path);
            }
        }
        paths.sort();
        let mut replay = Self {
            scope: self.scope,
            forms: self.forms,
            key: self.key,
            after: 0,
            observed_ms: 0,
        };
        let mut entries = Vec::new();
        let mut used = 0usize;
        let mut committed = 0u32;
        for path in paths {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                .open(&path)?;
            if !file.metadata()?.is_file() {
                return Err("history archive requires regular batch files".into());
            }
            let mut bytes = Vec::new();
            // Reject before retaining beyond the supplied byte envelope.
            let remaining = limits.bytes - used;
            (&mut file)
                .take(remaining.min(2048) as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > remaining || bytes.len() > 2048 {
                return Err("history archive byte capacity".into());
            }
            let (after, clock) = replay.validate(&bytes)?;
            if after == replay.after
                || path.file_name().unwrap()
                    != std::ffi::OsStr::new(&format!(
                        "observed-history-{:04}-{after:04}.json",
                        replay.after
                    ))
            {
                return Err("history archive filename or empty batch".into());
            }
            committed = committed.max(serde_json::from_slice::<Batch>(&bytes)?.committed);
            used += bytes.len();
            entries.push(Entry {
                after: replay.after,
                bytes: bytes.into_boxed_slice(),
            });
            replay.after = after;
            replay.observed_ms = clock;
        }
        if entries.is_empty() || committed != replay.after {
            return Err("history archive has no complete signed prefix".into());
        }
        Ok(Archive {
            entries: entries.into_boxed_slice(),
            revoked: false,
        })
    }
}
impl Archive {
    /// Owned layout bytes retained until retirement, including descriptor storage.
    /// Native admission must additionally cover its allocator and owner overhead;
    /// a payload limit alone is not an admitted resident-memory claim.
    pub(crate) fn retained_layout_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + std::mem::size_of_val(self.entries.as_ref())
            + self
                .entries
                .iter()
                .map(|entry| entry.bytes.len())
                .sum::<usize>()
    }
}
impl HistoryProvider for Archive {
    fn retained_layout_bytes(&self) -> usize {
        Archive::retained_layout_bytes(self)
    }
    fn read_signed_batch(
        &mut self,
        cursor: u32,
        output: &mut [u8; 2048],
    ) -> std::result::Result<usize, HistoryProviderRefusal> {
        if self.revoked {
            return Err(HistoryProviderRefusal::Unavailable);
        }
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.after == cursor)
            .ok_or(HistoryProviderRefusal::InvalidCursor)?;
        output[..entry.bytes.len()].copy_from_slice(&entry.bytes);
        Ok(entry.bytes.len())
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
