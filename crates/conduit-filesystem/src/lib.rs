//! Linux-class filesystem providers with explicit opaque resource bindings.
//!
//! Semantic panels contain only resource identities. Operating-system paths
//! are provider installation facts supplied through [`LinuxFilesystem`].

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use conduit_core::{Id, SemanticHash};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value, file_read_contract, file_watch_contract, file_write_contract,
};
use conduit_std::{FlushClaim, PartialWritePolicy, WatchEventKind, WriteMode};

pub const LINUX_FILESYSTEM_MAX_RESOURCES: usize = 16;
pub const LINUX_FILESYSTEM_MAX_SCAN_ENTRIES: usize = 256;
pub const LINUX_FILESYSTEM_MAX_OPERATION_BYTES: usize = conduit_std::FILESYSTEM_MAX_FILE_BYTES;
pub const LINUX_FILESYSTEM_MAX_WATCH_EVENTS: usize = conduit_std::FILESYSTEM_MAX_WATCH_EVENTS;

const MONOTONIC_CLOCK: &str = "conduit.clock/monotonic-ticks";
const MONOTONIC_CLOCK_HASH: &[u8; 32] = &[
    0x6b, 0x9c, 0x68, 0x72, 0x26, 0xd4, 0xa1, 0x96, 0x5e, 0x78, 0x0b, 0x63, 0xb4, 0xbd, 0xc0, 0x92,
    0x2d, 0xe2, 0xa6, 0x86, 0xc3, 0xc1, 0x36, 0x5f, 0x4f, 0x68, 0xf7, 0x21, 0x9f, 0x30, 0xcc, 0x48,
];

pub const EXAMPLE_READ_RESOURCE: &str = "conduit.resource/filesystem-example-read";
pub const EXAMPLE_WRITE_RESOURCE: &str = "conduit.resource/filesystem-example-write";
pub const EXAMPLE_WATCH_RESOURCE: &str = "conduit.resource/filesystem-example-watch";

/// Provider-owned mapping for one opaque semantic resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxResourceBinding {
    pub resource: String,
    pub path: PathBuf,
    pub scope_root: PathBuf,
    pub readable: bool,
    pub writable: bool,
    pub watchable: bool,
    pub sensitive: bool,
}

/// Stable hosted-provider failure reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxFilesystemError {
    InvalidResource,
    DuplicateResource,
    TooManyResources,
    RelativeProviderPath,
    OutsideScope,
    Denied,
    Missing,
    SymlinkRejected,
    WrongKind,
    InvalidBound,
    Oversized,
    AlreadyExists,
    PartialWrite,
    DurabilityUnsupported,
    Io,
    ScanOverflow,
    IdentityLost,
    Cancelled,
}

impl LinuxFilesystemError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidResource => "CND-FSH-001",
            Self::DuplicateResource => "CND-FSH-002",
            Self::TooManyResources => "CND-FSH-003",
            Self::RelativeProviderPath => "CND-FSH-004",
            Self::OutsideScope => "CND-FSH-005",
            Self::Denied => "CND-FSH-006",
            Self::Missing => "CND-FSH-007",
            Self::SymlinkRejected => "CND-FSH-008",
            Self::WrongKind => "CND-FSH-009",
            Self::InvalidBound => "CND-FSH-010",
            Self::Oversized => "CND-FSH-011",
            Self::AlreadyExists => "CND-FSH-012",
            Self::PartialWrite => "CND-FSH-013",
            Self::DurabilityUnsupported => "CND-FSH-014",
            Self::Io => "CND-FSH-015",
            Self::ScanOverflow => "CND-FSH-016",
            Self::IdentityLost => "CND-FSH-017",
            Self::Cancelled => "CND-FSH-018",
        }
    }
}

/// One bounded hosted read result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxRead {
    pub bytes: Vec<u8>,
    pub next_offset: u64,
    pub generation: u64,
    pub eof: bool,
}

/// One bounded hosted write result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxWrite {
    pub bytes_written: usize,
    pub generation: u64,
    pub committed: bool,
    pub complete: bool,
    pub flush: FlushClaim,
}

/// Provider-observed identity used to reconcile watch changes and renames.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxWatchState {
    resource: String,
    current_path: PathBuf,
    device: u64,
    inode: u64,
    length: u64,
    modified_ns: u128,
    generation: u64,
    emitted: usize,
    maximum_events: usize,
    cancelled: bool,
}

/// One hosted watch observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxWatchEvent {
    pub kind: WatchEventKind,
    pub resource: String,
    pub generation: u64,
}

/// Explicit finite Linux resource table.
pub struct LinuxFilesystem {
    resources: BTreeMap<String, LinuxResourceBinding>,
    maximum_operation_bytes: usize,
    maximum_scan_entries: usize,
}

impl LinuxFilesystem {
    pub fn new(
        bindings: Vec<LinuxResourceBinding>,
        maximum_operation_bytes: usize,
        maximum_scan_entries: usize,
    ) -> Result<Self, LinuxFilesystemError> {
        if bindings.len() > LINUX_FILESYSTEM_MAX_RESOURCES {
            return Err(LinuxFilesystemError::TooManyResources);
        }
        if maximum_operation_bytes == 0
            || maximum_operation_bytes > LINUX_FILESYSTEM_MAX_OPERATION_BYTES
            || maximum_scan_entries == 0
            || maximum_scan_entries > LINUX_FILESYSTEM_MAX_SCAN_ENTRIES
        {
            return Err(LinuxFilesystemError::InvalidBound);
        }
        let mut resources = BTreeMap::new();
        for binding in bindings {
            Id::new(&binding.resource).map_err(|_| LinuxFilesystemError::InvalidResource)?;
            if !binding.path.is_absolute() || !binding.scope_root.is_absolute() {
                return Err(LinuxFilesystemError::RelativeProviderPath);
            }
            if !binding.path.starts_with(&binding.scope_root) {
                return Err(LinuxFilesystemError::OutsideScope);
            }
            if resources
                .insert(binding.resource.clone(), binding)
                .is_some()
            {
                return Err(LinuxFilesystemError::DuplicateResource);
            }
        }
        Ok(Self {
            resources,
            maximum_operation_bytes,
            maximum_scan_entries,
        })
    }

    pub fn read(
        &self,
        resource: &str,
        offset: u64,
        maximum_bytes: usize,
        chunk_bytes: usize,
    ) -> Result<LinuxRead, LinuxFilesystemError> {
        if maximum_bytes == 0
            || chunk_bytes == 0
            || chunk_bytes > maximum_bytes
            || maximum_bytes > self.maximum_operation_bytes
        {
            return Err(LinuxFilesystemError::InvalidBound);
        }
        let binding = self.binding(resource)?;
        if !binding.readable {
            return Err(LinuxFilesystemError::Denied);
        }
        reject_final_symlink(&binding.path)?;
        let mut file = open_read_no_follow(&binding.path)?;
        let metadata = file.metadata().map_err(map_io)?;
        if !metadata.is_file() {
            return Err(LinuxFilesystemError::WrongKind);
        }
        file.seek(SeekFrom::Start(offset)).map_err(map_io)?;
        let mut bytes = vec![0; chunk_bytes];
        let count = file.read(&mut bytes).map_err(map_io)?;
        bytes.truncate(count);
        let next_offset = offset
            .checked_add(count as u64)
            .ok_or(LinuxFilesystemError::InvalidBound)?;
        Ok(LinuxRead {
            bytes,
            next_offset,
            generation: metadata_generation(&metadata),
            eof: next_offset >= metadata.len(),
        })
    }

    pub fn write(
        &self,
        resource: &str,
        mode: WriteMode,
        maximum_bytes: usize,
        partial: PartialWritePolicy,
        flush: FlushClaim,
        input: &[u8],
    ) -> Result<LinuxWrite, LinuxFilesystemError> {
        if maximum_bytes == 0 || maximum_bytes > self.maximum_operation_bytes {
            return Err(LinuxFilesystemError::InvalidBound);
        }
        if flush == FlushClaim::Durable {
            return Err(LinuxFilesystemError::DurabilityUnsupported);
        }
        let binding = self.binding(resource)?;
        if !binding.writable {
            return Err(LinuxFilesystemError::Denied);
        }
        if binding.path.exists() {
            reject_final_symlink(&binding.path)?;
        }
        let accepted = input.len().min(maximum_bytes);
        if accepted != input.len() && partial == PartialWritePolicy::FailWithoutCommit {
            return Err(LinuxFilesystemError::Oversized);
        }
        let existed = binding.path.exists();
        if mode == WriteMode::Create && existed {
            return Err(LinuxFilesystemError::AlreadyExists);
        }
        if mode != WriteMode::Create && !existed {
            return Err(LinuxFilesystemError::Missing);
        }
        let mut options = OpenOptions::new();
        options.write(true).custom_flags(libc::O_NOFOLLOW);
        match mode {
            WriteMode::Create => {
                options.create_new(true);
            }
            WriteMode::Replace => {
                options.truncate(true);
            }
            WriteMode::Append => {
                options.append(true);
            }
        }
        let mut file = options.open(&binding.path).map_err(map_io)?;
        let mut written = 0;
        while written < accepted {
            let count = file.write(&input[written..accepted]).map_err(map_io)?;
            if count == 0 {
                if partial == PartialWritePolicy::FailWithoutCommit {
                    return Err(LinuxFilesystemError::PartialWrite);
                }
                break;
            }
            written += count;
        }
        if flush == FlushClaim::ProviderAccepted {
            file.flush().map_err(map_io)?;
        }
        let metadata = file.metadata().map_err(map_io)?;
        Ok(LinuxWrite {
            bytes_written: written,
            generation: metadata_generation(&metadata),
            committed: written > 0 || input.is_empty(),
            complete: written == input.len(),
            flush,
        })
    }

    pub fn begin_watch(
        &self,
        resource: &str,
        maximum_events: usize,
    ) -> Result<LinuxWatchState, LinuxFilesystemError> {
        if maximum_events == 0 {
            return Err(LinuxFilesystemError::InvalidBound);
        }
        let binding = self.binding(resource)?;
        if !binding.watchable {
            return Err(LinuxFilesystemError::Denied);
        }
        reject_final_symlink(&binding.path)?;
        let metadata = fs::metadata(&binding.path).map_err(map_io)?;
        Ok(LinuxWatchState {
            resource: resource.to_owned(),
            current_path: binding.path.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified_ns: modified_ns(&metadata),
            generation: metadata_generation(&metadata),
            emitted: 0,
            maximum_events,
            cancelled: false,
        })
    }

    pub fn initial_watch_event(
        &self,
        state: &mut LinuxWatchState,
    ) -> Result<LinuxWatchEvent, LinuxFilesystemError> {
        state.record(WatchEventKind::Initial)
    }

    pub fn poll_watch(
        &self,
        state: &mut LinuxWatchState,
    ) -> Result<Option<LinuxWatchEvent>, LinuxFilesystemError> {
        if state.cancelled {
            return Err(LinuxFilesystemError::Cancelled);
        }
        let binding = self.binding(&state.resource)?;
        match fs::symlink_metadata(&state.current_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(LinuxFilesystemError::SymlinkRejected)
            }
            Ok(metadata) if metadata.dev() == state.device && metadata.ino() == state.inode => {
                let changed =
                    metadata.len() != state.length || modified_ns(&metadata) != state.modified_ns;
                if !changed {
                    return Ok(None);
                }
                state.length = metadata.len();
                state.modified_ns = modified_ns(&metadata);
                state.generation = metadata_generation(&metadata);
                state.record(WatchEventKind::Changed).map(Some)
            }
            Ok(_) => state.record(WatchEventKind::Removed).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let renamed = find_identity(
                    &binding.scope_root,
                    state.device,
                    state.inode,
                    self.maximum_scan_entries,
                )?;
                if let Some(path) = renamed {
                    state.current_path = path;
                    state.generation = state.generation.saturating_add(1);
                    state.record(WatchEventKind::Renamed).map(Some)
                } else {
                    state.record(WatchEventKind::Removed).map(Some)
                }
            }
            Err(_) => Err(LinuxFilesystemError::Io),
        }
    }

    pub fn cancel_watch(&self, state: &mut LinuxWatchState) -> Result<(), LinuxFilesystemError> {
        state.cancelled = true;
        Ok(())
    }

    #[must_use]
    pub fn evidence_resource(&self, resource: &str) -> Option<&str> {
        self.resources.get(resource).map(|binding| {
            if binding.sensitive {
                "<redacted-resource>"
            } else {
                binding.resource.as_str()
            }
        })
    }

    fn binding(&self, resource: &str) -> Result<&LinuxResourceBinding, LinuxFilesystemError> {
        self.resources
            .get(resource)
            .ok_or(LinuxFilesystemError::InvalidResource)
    }
}

impl LinuxWatchState {
    fn record(&mut self, kind: WatchEventKind) -> Result<LinuxWatchEvent, LinuxFilesystemError> {
        if self.cancelled {
            return Err(LinuxFilesystemError::Cancelled);
        }
        if self.emitted >= self.maximum_events {
            return Err(LinuxFilesystemError::ScanOverflow);
        }
        self.emitted += 1;
        Ok(LinuxWatchEvent {
            kind,
            resource: self.resource.clone(),
            generation: self.generation,
        })
    }
}

fn reject_final_symlink(path: &Path) -> Result<(), LinuxFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(LinuxFilesystemError::SymlinkRejected)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(LinuxFilesystemError::Missing)
        }
        Err(_) => Err(LinuxFilesystemError::Io),
    }
}

fn open_read_no_follow(path: &Path) -> Result<File, LinuxFilesystemError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(map_io)
}

fn map_io(error: std::io::Error) -> LinuxFilesystemError {
    match error.kind() {
        std::io::ErrorKind::NotFound => LinuxFilesystemError::Missing,
        std::io::ErrorKind::AlreadyExists => LinuxFilesystemError::AlreadyExists,
        std::io::ErrorKind::PermissionDenied => LinuxFilesystemError::Denied,
        _ => LinuxFilesystemError::Io,
    }
}

fn modified_ns(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn metadata_generation(metadata: &fs::Metadata) -> u64 {
    metadata
        .ctime()
        .unsigned_abs()
        .saturating_mul(1_000_000_000)
        .saturating_add(metadata.ctime_nsec().unsigned_abs())
        ^ metadata.len()
        ^ metadata.ino()
}

fn find_identity(
    root: &Path,
    device: u64,
    inode: u64,
    maximum_entries: usize,
) -> Result<Option<PathBuf>, LinuxFilesystemError> {
    let mut seen = 0;
    for entry in fs::read_dir(root).map_err(map_io)? {
        seen += 1;
        if seen > maximum_entries {
            return Err(LinuxFilesystemError::ScanOverflow);
        }
        let entry = entry.map_err(|_| LinuxFilesystemError::Io)?;
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|_| LinuxFilesystemError::Io)?;
        if !metadata.file_type().is_symlink() && metadata.dev() == device && metadata.ino() == inode
        {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

fn required_reference<'a>(node: &'a Node, key: &str) -> Result<&'a str, RuntimeError> {
    match node.config_value(key) {
        Some(
            SourceValue::Reference(value)
            | SourceValue::ContractReference(value)
            | SourceValue::SecretReference(value),
        ) => Ok(value),
        _ => Err(RuntimeError::new(
            "CND-FSH-019",
            format!("file node `{}` has no exact `{key}` reference", node.id),
        )),
    }
}

fn exact_secret(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    )
}

fn required_usize(node: &Node, key: &str) -> Result<usize, RuntimeError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => usize::try_from(*value).map_err(|_| {
            RuntimeError::new(
                "CND-FSH-010",
                format!("file node `{}` has invalid `{key}`", node.id),
            )
        }),
        _ => Err(RuntimeError::new(
            "CND-FSH-010",
            format!("file node `{}` has no exact `{key}`", node.id),
        )),
    }
}

fn example_filesystem() -> Result<LinuxFilesystem, LinuxFilesystemError> {
    let fixture = fs::canonicalize(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/filesystem/read-source.txt"
    ))
    .map_err(map_io)?;
    let fixture_root = fixture
        .parent()
        .ok_or(LinuxFilesystemError::OutsideScope)?
        .to_path_buf();
    let target =
        fs::canonicalize(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).map_err(map_io)?;
    Ok(LinuxFilesystem::new(
        vec![
            LinuxResourceBinding {
                resource: EXAMPLE_READ_RESOURCE.to_owned(),
                path: fixture.clone(),
                scope_root: fixture_root.clone(),
                readable: true,
                writable: false,
                watchable: false,
                sensitive: false,
            },
            LinuxResourceBinding {
                resource: EXAMPLE_WRITE_RESOURCE.to_owned(),
                path: target.join("conduit-filesystem-example.bin"),
                scope_root: target.clone(),
                readable: true,
                writable: true,
                watchable: false,
                sensitive: true,
            },
            LinuxResourceBinding {
                resource: EXAMPLE_WATCH_RESOURCE.to_owned(),
                path: fixture,
                scope_root: fixture_root,
                readable: true,
                writable: false,
                watchable: true,
                sensitive: false,
            },
        ],
        LINUX_FILESYSTEM_MAX_OPERATION_BYTES,
        LINUX_FILESYSTEM_MAX_SCAN_ENTRIES,
    )?)
}

fn read_result_bytes(read: &LinuxRead) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(25);
    bytes.extend_from_slice(&(read.bytes.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&read.next_offset.to_be_bytes());
    bytes.extend_from_slice(&read.generation.to_be_bytes());
    bytes.push(u8::from(read.eof));
    bytes
}

fn write_result_bytes(write: LinuxWrite) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(19);
    bytes.extend_from_slice(&(write.bytes_written as u64).to_be_bytes());
    bytes.extend_from_slice(&write.generation.to_be_bytes());
    bytes.push(u8::from(write.committed));
    bytes.push(u8::from(write.complete));
    bytes.push(match write.flush {
        FlushClaim::None => 0,
        FlushClaim::ProviderAccepted => 1,
        FlushClaim::Durable => 2,
    });
    bytes
}

fn watch_event_bytes(event: &LinuxWatchEvent) -> Vec<u8> {
    let resource = event.resource.as_bytes();
    let mut bytes = Vec::with_capacity(11 + resource.len());
    bytes.push(match event.kind {
        WatchEventKind::Initial => 0,
        WatchEventKind::Created => 1,
        WatchEventKind::Changed => 2,
        WatchEventKind::Removed => 3,
        WatchEventKind::Renamed => 4,
        WatchEventKind::Gap => 5,
        WatchEventKind::Resync => 6,
    });
    bytes.extend_from_slice(&event.generation.to_be_bytes());
    bytes.extend_from_slice(&(resource.len() as u16).to_be_bytes());
    bytes.extend_from_slice(resource);
    bytes
}

fn validate_read_config(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "offset",
            "maximum_bytes",
            "chunk_bytes",
            "consistency",
            "eof",
            "cancellation",
        ],
    )?;
    if !exact_secret(node, "resource", EXAMPLE_READ_RESOURCE)
        || !exact_secret(node, "grant", "conduit.grant/filesystem-read")
        || !matches!(node.config("consistency"), Some("snapshot" | "live"))
        || node.config("eof") != Some("terminal")
        || node.config("cancellation") != Some("discard")
    {
        return Err(ResolutionError::new(
            "CND-FSH-019",
            format!("file read `{}` has unsupported semantics", node.id),
        ));
    }
    validate_bound(node, "maximum_bytes")?;
    validate_bound(node, "chunk_bytes")?;
    Ok(())
}

fn validate_write_config(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "mode",
            "maximum_bytes",
            "partial",
            "flush",
            "cleanup",
            "cancellation",
        ],
    )?;
    if !exact_secret(node, "resource", EXAMPLE_WRITE_RESOURCE)
        || !exact_secret(node, "grant", "conduit.grant/filesystem-write")
        || !matches!(node.config("mode"), Some("create" | "replace" | "append"))
        || !matches!(
            node.config("partial"),
            Some("fail-without-commit" | "report-committed-prefix")
        )
        || !matches!(node.config("flush"), Some("none" | "provider-accepted"))
        || node.config("cleanup") != Some("close")
        || node.config("cancellation") != Some("close")
    {
        return Err(ResolutionError::new(
            "CND-FSH-019",
            format!("file write `{}` has unsupported semantics", node.id),
        ));
    }
    validate_bound(node, "maximum_bytes")
}

fn validate_watch_config(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "clock",
            "clock_schema_version",
            "clock_hash",
            "event_kinds",
            "emit_initial",
            "coalescing",
            "loss",
            "queue_capacity",
            "maximum_events",
            "overflow",
            "rename_identity",
            "cancellation",
        ],
    )?;
    if !exact_secret(node, "resource", EXAMPLE_WATCH_RESOURCE)
        || !exact_secret(node, "grant", "conduit.grant/filesystem-watch")
        || node.config("clock") != Some(MONOTONIC_CLOCK)
        || !matches!(
            node.config_value("clock_schema_version"),
            Some(SourceValue::Integer(0))
        )
        || !matches!(
            node.config_value("clock_hash"),
            Some(SourceValue::Bytes(hash)) if hash.as_slice() == MONOTONIC_CLOCK_HASH
        )
        || node.config("event_kinds") != Some("create-change-remove-rename")
        || !matches!(
            node.config_value("emit_initial"),
            Some(SourceValue::Boolean(true))
        )
        || !matches!(
            node.config("coalescing"),
            Some("none" | "same-handle-latest")
        )
        || node.config("loss") != Some("explicit-gap")
        || !matches!(node.config("overflow"), Some("terminal-gap" | "gap-resync"))
        || node.config("rename_identity") != Some("preserve-handle")
        || node.config("cancellation") != Some("close")
    {
        return Err(ResolutionError::new(
            "CND-FSH-019",
            format!("file watch `{}` has unsupported semantics", node.id),
        ));
    }
    validate_watch_bound(node, "queue_capacity")?;
    validate_watch_bound(node, "maximum_events")
}

fn validate_exact_keys(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len()
        || expected
            .iter()
            .any(|key| !node.config.iter().any(|entry| entry.key == *key))
    {
        return Err(ResolutionError::new(
            "CND-FSH-019",
            format!("file node `{}` does not match its exact config", node.id),
        ));
    }
    Ok(())
}

fn validate_bound(node: &Node, key: &str) -> Result<(), ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value))
            if *value > 0 && *value <= LINUX_FILESYSTEM_MAX_OPERATION_BYTES as i128 =>
        {
            Ok(())
        }
        _ => Err(ResolutionError::new(
            "CND-FSH-010",
            format!("file node `{}` has invalid `{key}`", node.id),
        )),
    }
}

fn validate_watch_bound(node: &Node, key: &str) -> Result<(), ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value))
            if *value > 0 && *value <= LINUX_FILESYSTEM_MAX_WATCH_EVENTS as i128 =>
        {
            Ok(())
        }
        _ => Err(ResolutionError::new(
            "CND-FSH-010",
            format!("file node `{}` has invalid `{key}`", node.id),
        )),
    }
}

struct ReadHandler;

impl Handler for ReadHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-FSH-019",
                "file read received hidden inputs",
            ));
        }
        validate_read_config(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let read = example_filesystem()
            .map_err(runtime_error)?
            .read(
                required_reference(node, "resource")?,
                required_usize(node, "offset")? as u64,
                required_usize(node, "maximum_bytes")?,
                required_usize(node, "chunk_bytes")?,
            )
            .map_err(runtime_error)?;
        Ok(vec![
            Value {
                value_type: file_read_contract().outputs[0].value_type,
                bytes: read.bytes.clone(),
            },
            Value {
                value_type: file_read_contract().outputs[1].value_type,
                bytes: read_result_bytes(&read),
            },
        ])
    }
}

struct WriteHandler;

impl Handler for WriteHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_write_config(node)
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let input = inputs
            .first()
            .filter(|input| input.value_type == file_write_contract().inputs[0].value_type)
            .ok_or_else(|| RuntimeError::new("CND-FSH-019", "file write chunk is missing"))?;
        let mode = match node.config("mode") {
            Some("create") => WriteMode::Create,
            Some("replace") => WriteMode::Replace,
            Some("append") => WriteMode::Append,
            _ => {
                return Err(RuntimeError::new(
                    "CND-FSH-019",
                    "file write mode disappeared",
                ));
            }
        };
        let partial = if node.config("partial") == Some("report-committed-prefix") {
            PartialWritePolicy::ReportCommittedPrefix
        } else {
            PartialWritePolicy::FailWithoutCommit
        };
        let flush = if node.config("flush") == Some("provider-accepted") {
            FlushClaim::ProviderAccepted
        } else {
            FlushClaim::None
        };
        let write = example_filesystem()
            .map_err(runtime_error)?
            .write(
                required_reference(node, "resource")?,
                mode,
                required_usize(node, "maximum_bytes")?,
                partial,
                flush,
                &input.bytes,
            )
            .map_err(runtime_error)?;
        Ok(vec![Value {
            value_type: file_write_contract().outputs[0].value_type,
            bytes: write_result_bytes(write),
        }])
    }
}

struct WatchHandler;

impl Handler for WatchHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-FSH-019",
                "file watch received hidden inputs",
            ));
        }
        validate_watch_config(node)
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let filesystem = example_filesystem().map_err(runtime_error)?;
        let mut state = filesystem
            .begin_watch(
                required_reference(node, "resource")?,
                required_usize(node, "maximum_events")?,
            )
            .map_err(runtime_error)?;
        let event = filesystem
            .initial_watch_event(&mut state)
            .map_err(runtime_error)?;
        Ok(vec![Value {
            value_type: file_watch_contract().outputs[0].value_type,
            bytes: watch_event_bytes(&event),
        }])
    }
}

fn runtime_error(error: LinuxFilesystemError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

pub fn register_hosted_file_read_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    static REQUIRED_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x31; 32])];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: file_read_contract(),
        implementation_id: "conduit/filesystem-linux-read",
        artifact_id: "conduit/filesystem-linux-read-artifact",
        entrypoint: "filesystem-linux-read",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &REQUIRED_AUTHORITIES,
        factory: || Box::new(ReadHandler),
        validate_config: validate_read_config,
    })
}

/// Explicitly install the dangerous write provider.
pub fn register_hosted_file_write_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    static REQUIRED_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x32; 32])];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: file_write_contract(),
        implementation_id: "conduit/filesystem-linux-write",
        artifact_id: "conduit/filesystem-linux-write-artifact",
        entrypoint: "filesystem-linux-write",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &REQUIRED_AUTHORITIES,
        factory: || Box::new(WriteHandler),
        validate_config: validate_write_config,
    })
}

/// Explicitly install the dangerous watch provider.
pub fn register_hosted_file_watch_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    static REQUIRED_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x33; 32])];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: file_watch_contract(),
        implementation_id: "conduit/filesystem-linux-watch",
        artifact_id: "conduit/filesystem-linux-watch-artifact",
        entrypoint: "filesystem-linux-watch",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &REQUIRED_AUTHORITIES,
        factory: || Box::new(WatchHandler),
        validate_config: validate_watch_config,
    })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;
    use conduit_runtime::AvailabilityState;
    use conduit_std::{
        FileHandle, FileSlot, MemoryFilesystem, ReadConsistency, ReadRequest, WriteRequest,
    };
    use tempfile::tempdir;

    fn binding(
        resource: &str,
        path: &Path,
        root: &Path,
        readable: bool,
        writable: bool,
        watchable: bool,
    ) -> LinuxResourceBinding {
        LinuxResourceBinding {
            resource: resource.to_owned(),
            path: path.to_path_buf(),
            scope_root: root.to_path_buf(),
            readable,
            writable,
            watchable,
            sensitive: false,
        }
    }

    fn filesystem(binding: LinuxResourceBinding) -> LinuxFilesystem {
        LinuxFilesystem::new(vec![binding], 16, 8).unwrap()
    }

    #[test]
    fn resource_table_rejects_ambient_relative_and_out_of_scope_paths() {
        let relative = LinuxResourceBinding {
            resource: "resource/file".to_owned(),
            path: PathBuf::from("file"),
            scope_root: PathBuf::from("."),
            readable: true,
            writable: false,
            watchable: false,
            sensitive: false,
        };
        assert!(matches!(
            LinuxFilesystem::new(vec![relative], 16, 8),
            Err(LinuxFilesystemError::RelativeProviderPath)
        ));

        let directory = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let escaped = binding(
            "resource/file",
            &outside.path().join("file"),
            directory.path(),
            true,
            false,
            false,
        );
        assert!(matches!(
            LinuxFilesystem::new(vec![escaped], 16, 8),
            Err(LinuxFilesystemError::OutsideScope)
        ));
    }

    #[test]
    fn read_range_eof_bounds_and_symlink_policy_are_exact() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file");
        fs::write(&path, b"abcdef").unwrap();
        let files = filesystem(binding(
            "resource/file",
            &path,
            directory.path(),
            true,
            false,
            false,
        ));
        let read = files.read("resource/file", 2, 4, 4).unwrap();
        assert_eq!(read.bytes, b"cdef");
        assert!(read.eof);
        assert_eq!(
            files.read("resource/file", 0, 17, 4),
            Err(LinuxFilesystemError::InvalidBound)
        );
        assert_eq!(
            files.read("resource/missing", 0, 4, 4),
            Err(LinuxFilesystemError::InvalidResource)
        );

        let link = directory.path().join("link");
        symlink(&path, &link).unwrap();
        let linked = filesystem(binding(
            "resource/link",
            &link,
            directory.path(),
            true,
            false,
            false,
        ));
        assert_eq!(
            linked.read("resource/link", 0, 4, 4),
            Err(LinuxFilesystemError::SymlinkRejected)
        );
    }

    #[test]
    fn create_replace_append_partial_and_flush_claims_are_honest() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file");
        let files = filesystem(binding(
            "resource/file",
            &path,
            directory.path(),
            true,
            true,
            false,
        ));
        let created = files
            .write(
                "resource/file",
                WriteMode::Create,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::ProviderAccepted,
                b"abc",
            )
            .unwrap();
        assert!(created.complete);
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        assert_eq!(
            files.write(
                "resource/file",
                WriteMode::Create,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::None,
                b"x",
            ),
            Err(LinuxFilesystemError::AlreadyExists)
        );
        files
            .write(
                "resource/file",
                WriteMode::Append,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::None,
                b"de",
            )
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abcde");
        files
            .write(
                "resource/file",
                WriteMode::Replace,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::None,
                b"z",
            )
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"z");
        assert_eq!(
            files.write(
                "resource/file",
                WriteMode::Replace,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::Durable,
                b"x",
            ),
            Err(LinuxFilesystemError::DurabilityUnsupported)
        );
        assert_eq!(
            files.write(
                "resource/file",
                WriteMode::Replace,
                2,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::None,
                b"long",
            ),
            Err(LinuxFilesystemError::Oversized)
        );
        assert_eq!(fs::read(&path).unwrap(), b"z");
        let partial = files
            .write(
                "resource/file",
                WriteMode::Replace,
                2,
                PartialWritePolicy::ReportCommittedPrefix,
                FlushClaim::None,
                b"long",
            )
            .unwrap();
        assert_eq!(partial.bytes_written, 2);
        assert!(!partial.complete);
        assert_eq!(fs::read(&path).unwrap(), b"lo");
    }

    #[test]
    fn watch_change_rename_remove_and_cancel_preserve_resource_identity() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file");
        fs::write(&path, b"a").unwrap();
        let files = filesystem(binding(
            "resource/file",
            &path,
            directory.path(),
            false,
            false,
            true,
        ));
        let mut watch = files.begin_watch("resource/file", 5).unwrap();
        assert_eq!(
            files.initial_watch_event(&mut watch).unwrap().kind,
            WatchEventKind::Initial
        );
        fs::write(&path, b"changed").unwrap();
        assert_eq!(
            files.poll_watch(&mut watch).unwrap().unwrap().kind,
            WatchEventKind::Changed
        );
        let renamed = directory.path().join("renamed");
        fs::rename(&path, &renamed).unwrap();
        let event = files.poll_watch(&mut watch).unwrap().unwrap();
        assert_eq!(event.kind, WatchEventKind::Renamed);
        assert_eq!(event.resource, "resource/file");
        fs::remove_file(&renamed).unwrap();
        assert_eq!(
            files.poll_watch(&mut watch).unwrap().unwrap().kind,
            WatchEventKind::Removed
        );
        files.cancel_watch(&mut watch).unwrap();
        assert_eq!(
            files.poll_watch(&mut watch),
            Err(LinuxFilesystemError::Cancelled)
        );
    }

    #[test]
    fn sensitive_provider_mapping_is_redacted_and_dangerous_services_are_opt_in() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("secret");
        fs::write(&path, b"secret").unwrap();
        let mut secret = binding(
            "resource/secret",
            &path,
            directory.path(),
            true,
            false,
            false,
        );
        secret.sensitive = true;
        let files = filesystem(secret);
        assert_eq!(
            files.evidence_resource("resource/secret"),
            Some("<redacted-resource>")
        );

        let mut registry = Registry::hosted_primitives();
        register_hosted_file_read_provider(&mut registry).unwrap();
        assert_eq!(
            registry.node_availability("fs/read").state,
            AvailabilityState::ProviderAvailable
        );
        assert_eq!(
            registry.node_availability("fs/write").state,
            AvailabilityState::ContractOnly
        );
        assert_eq!(
            registry.node_availability("fs/watch").state,
            AvailabilityState::ContractOnly
        );
        register_hosted_file_write_provider(&mut registry).unwrap();
        register_hosted_file_watch_provider(&mut registry).unwrap();
        assert_eq!(
            registry.node_availability("fs/write").state,
            AvailabilityState::ProviderAvailable
        );
        assert_eq!(
            registry.node_availability("fs/watch").state,
            AvailabilityState::ProviderAvailable
        );
    }

    #[test]
    fn deterministic_and_linux_providers_agree_on_normalized_read_and_write_semantics() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file");
        fs::write(&path, b"abcdef").unwrap();
        let linux = filesystem(binding(
            "resource/file",
            &path,
            directory.path(),
            true,
            true,
            false,
        ));
        let linux_read = linux.read("resource/file", 2, 4, 4).unwrap();

        let mut memory =
            MemoryFilesystem::<1, 16, 4>::new([
                FileSlot::seeded(FileHandle(1), b"abcdef", false).unwrap()
            ]);
        let mut bytes = [0; 4];
        let memory_read = memory
            .read(
                ReadRequest {
                    handle: FileHandle(1),
                    offset: 2,
                    maximum_bytes: 4,
                    chunk_bytes: 4,
                    consistency: ReadConsistency::Snapshot,
                },
                &mut bytes,
            )
            .unwrap();
        assert_eq!(linux_read.bytes, bytes);
        assert_eq!(linux_read.next_offset, memory_read.next_offset);
        assert_eq!(linux_read.eof, memory_read.eof);

        let linux_write = linux
            .write(
                "resource/file",
                WriteMode::Replace,
                8,
                PartialWritePolicy::FailWithoutCommit,
                FlushClaim::ProviderAccepted,
                b"updated",
            )
            .unwrap();
        let memory_write = memory
            .write(
                WriteRequest {
                    handle: FileHandle(1),
                    mode: WriteMode::Replace,
                    maximum_bytes: 8,
                    partial: PartialWritePolicy::FailWithoutCommit,
                    requested_flush: FlushClaim::ProviderAccepted,
                },
                b"updated",
            )
            .unwrap();
        assert_eq!(linux_write.bytes_written, memory_write.bytes_written);
        assert_eq!(linux_write.committed, memory_write.committed);
        assert_eq!(linux_write.complete, memory_write.complete);
        assert_eq!(linux_write.flush, memory_write.flush);
        assert_eq!(fs::read(path).unwrap(), b"updated");
    }
}
