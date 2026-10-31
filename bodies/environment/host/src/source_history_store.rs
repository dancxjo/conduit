//! Finite local collector evidence, not MCU membership or transport acknowledgments.
//!
//! The installer must admit the directory, byte/record bound and blocking I/O
//! worker before Play. This store cannot establish a selected recorder commit;
//! callers append only bytes delivered by their actual committed publication.
use conduit_environment_runtime::environmental_reading::source_observation::ObservationContract;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
    path::{Path, PathBuf},
};

const MAX_RECORD_BYTES: usize = 2048;
const MAX_SOURCES: usize = 4;
const VISIBLE: usize = 16;
type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Invalid,
    Capacity,
    ForeignScope,
    ClockReversed,
    Uncertain,
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectorScope {
    pub host: String,
    pub boot: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSession {
    pub source: String,
    pub session: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Process-local execution coordinates supplied by the owning committed API.
/// These caller-provided names are not a cryptographic proof or portable receipt.
pub struct ExecutionProvenance {
    pub plan: String,
    pub play: String,
    pub source: String,
    pub checked: String,
    pub expanded: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub collector: CollectorScope,
    pub source: SourceSession,
    pub sequence: u64,
    pub execution: ExecutionProvenance,
    /// Exact canonical SourceObservation, including collector Boot coordinate.
    pub observation: Vec<u8>,
}
pub struct Series {
    pub source: SourceSession,
    pub samples_outside_view: u64,
    pub records: VecDeque<Record>,
}
/// Immutable records never wrap or disappear at capacity. Only the in-memory
/// presentation window truncates; its omitted count remains explicit.
pub struct SourceHistoryStore {
    directory: PathBuf,
    _lock: File,
    scope: CollectorScope,
    capacity: u64,
    next: u64,
    last_ms: Option<u64>,
    series: Vec<Series>,
    contract: ObservationContract,
    uncertain: bool,
}
fn identity(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
impl SourceHistoryStore {
    /// Opens an existing private directory. A new collector Boot uses a new
    /// directory; old Boot histories remain readable under their original scope.
    /// `capacity * 2048` bounds retained record bytes, plus filesystem overhead.
    pub fn open(directory: &Path, scope: CollectorScope, capacity: u64) -> Result<Self> {
        if !identity(&scope.host) || !identity(&scope.boot) || !(1..=65536).contains(&capacity) {
            return Err(Error::Invalid);
        }
        let metadata = fs::symlink_metadata(directory)?;
        use std::os::unix::fs::MetadataExt;
        if !metadata.is_dir()
            || metadata.mode() & 0o077 != 0
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(Error::Invalid);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join("lock"))?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut store = Self {
            directory: directory.into(),
            _lock: lock,
            scope,
            capacity,
            next: 1,
            last_ms: None,
            series: Vec::with_capacity(MAX_SOURCES),
            contract: ObservationContract::prepare().map_err(|_| Error::Invalid)?,
            uncertain: false,
        };
        // Directory enumeration itself is bounded, including unexpected files.
        let mut count = 0u64;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == "lock" {
                continue;
            }
            if name == ".pending" {
                fs::remove_file(entry.path())?;
                continue;
            }
            count += 1;
            if count > capacity {
                return Err(Error::Capacity);
            }
            let name = name.to_str().ok_or(Error::Invalid)?.as_bytes();
            if name.len() != 24
                || !name.ends_with(b".json")
                || !name[..19].iter().all(u8::is_ascii_digit)
            {
                return Err(Error::Invalid);
            }
        }
        for sequence in 1..=count {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(store.path(sequence))?;
            let mut bytes = Vec::new();
            file.take((MAX_RECORD_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes.len() > MAX_RECORD_BYTES {
                return Err(Error::Invalid);
            }
            let record: Record = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
            store.validate(&record)?;
            store.retain(record);
        }
        File::open(directory)?.sync_all()?;
        Ok(store)
    }
    fn path(&self, sequence: u64) -> PathBuf {
        self.directory.join(format!("{sequence:019}.json"))
    }
    fn validate(&self, record: &Record) -> Result<()> {
        if record.collector != self.scope {
            return Err(Error::ForeignScope);
        }
        let expected_sequence = self
            .series
            .iter()
            .find(|s| s.source == record.source)
            .and_then(|s| s.records.back())
            .map_or(Some(1), |r| r.sequence.checked_add(1))
            .ok_or(Error::Invalid)?;
        if record.sequence != expected_sequence {
            return Err(Error::Invalid);
        }
        if !identity(&record.source.source) || !identity(&record.source.session) {
            return Err(Error::Invalid);
        }
        if [
            &record.execution.plan,
            &record.execution.play,
            &record.execution.source,
            &record.execution.checked,
            &record.execution.expanded,
        ]
        .iter()
        .any(|v| !identity(v))
        {
            return Err(Error::Invalid);
        }
        let observation = self
            .contract
            .decode(&record.observation)
            .map_err(|_| Error::Invalid)?;
        if self
            .last_ms
            .is_some_and(|ms| observation.observed_at_host_boot_ms < ms)
        {
            return Err(Error::ClockReversed);
        }
        if !self.series.iter().any(|s| s.source == record.source)
            && self.series.len() == MAX_SOURCES
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    fn retain(&mut self, record: Record) {
        self.last_ms = Some(
            self.contract
                .decode(&record.observation)
                .expect("validated observation")
                .observed_at_host_boot_ms,
        );
        self.next += 1;
        let index = self
            .series
            .iter()
            .position(|s| s.source == record.source)
            .unwrap_or_else(|| {
                self.series.push(Series {
                    source: record.source.clone(),
                    samples_outside_view: 0,
                    records: VecDeque::with_capacity(VISIBLE),
                });
                self.series.len() - 1
            });
        let series = &mut self.series[index];
        if series.records.len() == VISIBLE {
            series.records.pop_front();
            series.samples_outside_view += 1;
        }
        series.records.push_back(record);
    }
    /// Success means this local immutable file and directory entry were synced.
    /// It is neither transport delivery ACK nor selected Body recorder receipt.
    /// On uncertain I/O, reopen and inspect; never retry blindly with a new ID.
    pub fn append_committed(
        &mut self,
        source: SourceSession,
        sequence: u64,
        execution: ExecutionProvenance,
        observation: &[u8],
    ) -> Result<u64> {
        if self.uncertain {
            return Err(Error::Uncertain);
        }
        if self.next > self.capacity {
            return Err(Error::Capacity);
        }
        let record = Record {
            collector: self.scope.clone(),
            source,
            sequence,
            execution,
            observation: observation.into(),
        };
        self.validate(&record)?;
        let bytes = serde_json::to_vec(&record).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(Error::Invalid);
        }
        let temporary = self.directory.join(".pending");
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            // No replacement of an occupied original sequence, even on retry.
            fs::hard_link(&temporary, self.path(self.next))?;
            File::open(&self.directory)?.sync_all()?;
            fs::remove_file(&temporary)?;
            File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            self.uncertain = true;
            return result.map(|_| 0);
        }
        let sequence = record.sequence;
        self.retain(record);
        Ok(sequence)
    }
    pub fn last_sequence(&self, source: &SourceSession) -> Option<u64> {
        self.series
            .iter()
            .find(|s| &s.source == source)
            .and_then(|s| s.records.back())
            .map(|r| r.sequence)
    }
    pub fn series(&self) -> &[Series] {
        &self.series
    }
    pub fn collector(&self) -> &CollectorScope {
        &self.scope
    }
    /// Number of retained immutable records across all source sessions.
    pub fn retained_records(&self) -> u64 {
        self.next - 1
    }
}

#[cfg(test)]
#[path = "source_history_store_tests.rs"]
mod tests;
