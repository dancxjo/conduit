//! Bounded filesystem semantics for deterministic providers.
//!
//! Resource handles are opaque tokens. This module never contains, expands,
//! joins, or resolves an operating-system path.

/// Maximum file slots required by the deterministic reference profile.
pub const FILESYSTEM_MAX_FILES: usize = 8;
/// Maximum bytes retained for one deterministic file.
pub const FILESYSTEM_MAX_FILE_BYTES: usize = 65_536;
/// Maximum queued watch observations in the reference profile.
pub const FILESYSTEM_MAX_WATCH_EVENTS: usize = 32;

/// Opaque provider-scoped file identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FileHandle(pub u32);

/// Consistency promised by one read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadConsistency {
    /// All chunks come from one generation.
    Snapshot,
    /// Each chunk may observe the current generation.
    Live,
}

/// One exact bounded read operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadRequest {
    pub handle: FileHandle,
    pub offset: u64,
    pub maximum_bytes: usize,
    pub chunk_bytes: usize,
    pub consistency: ReadConsistency,
}

/// Result metadata accompanying one read chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadResult {
    pub bytes_read: usize,
    pub next_offset: u64,
    pub generation: u64,
    pub eof: bool,
}

/// File mutation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteMode {
    Create,
    Replace,
    Append,
}

/// Behavior when the provider cannot accept the whole declared write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartialWritePolicy {
    FailWithoutCommit,
    ReportCommittedPrefix,
}

/// Strength of the provider's flush claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlushClaim {
    None,
    ProviderAccepted,
    Durable,
}

/// One exact bounded write operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteRequest {
    pub handle: FileHandle,
    pub mode: WriteMode,
    pub maximum_bytes: usize,
    pub partial: PartialWritePolicy,
    pub requested_flush: FlushClaim,
}

/// Exact write outcome without false atomicity or durability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteResult {
    pub bytes_written: usize,
    pub generation: u64,
    pub committed: bool,
    pub complete: bool,
    pub flush: FlushClaim,
}

/// Watch event kinds shared by deterministic and hosted providers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchEventKind {
    Initial,
    Created,
    Changed,
    Removed,
    Renamed,
    Gap,
    Resync,
}

/// Coalescing semantics for a watch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchCoalescing {
    None,
    SameHandleLatest,
}

/// Overflow behavior for a finite watch queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchOverflow {
    TerminalGap,
    GapThenResync,
}

/// One exact bounded watch policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchRequest {
    pub emit_initial: bool,
    pub maximum_events: usize,
    pub queue_capacity: usize,
    pub coalescing: WatchCoalescing,
    pub overflow: WatchOverflow,
}

/// One ordered provider observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    pub sequence: u64,
    pub kind: WatchEventKind,
    pub handle: FileHandle,
    pub generation: u64,
}

/// Stable filesystem failure reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemError {
    InvalidBound,
    WrongHandle,
    Missing,
    AlreadyExists,
    OffsetOverflow,
    Oversized,
    PartialWrite,
    DurabilityUnsupported,
    WatchInactive,
    WatchExhausted,
    WatchGap,
    Cancelled,
}

impl FilesystemError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidBound => "CND-FS-001",
            Self::WrongHandle => "CND-FS-002",
            Self::Missing => "CND-FS-003",
            Self::AlreadyExists => "CND-FS-004",
            Self::OffsetOverflow => "CND-FS-005",
            Self::Oversized => "CND-FS-006",
            Self::PartialWrite => "CND-FS-007",
            Self::DurabilityUnsupported => "CND-FS-008",
            Self::WatchInactive => "CND-FS-009",
            Self::WatchExhausted => "CND-FS-010",
            Self::WatchGap => "CND-FS-011",
            Self::Cancelled => "CND-FS-012",
        }
    }
}

/// One fixed-capacity deterministic file slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSlot<const BYTES: usize> {
    pub handle: FileHandle,
    bytes: [u8; BYTES],
    length: usize,
    generation: u64,
    exists: bool,
    sensitive: bool,
}

impl<const BYTES: usize> FileSlot<BYTES> {
    #[must_use]
    pub const fn empty(handle: FileHandle, sensitive: bool) -> Self {
        Self {
            handle,
            bytes: [0; BYTES],
            length: 0,
            generation: 0,
            exists: false,
            sensitive,
        }
    }

    #[must_use]
    pub fn seeded(
        handle: FileHandle,
        value: &[u8],
        sensitive: bool,
    ) -> Result<Self, FilesystemError> {
        if value.len() > BYTES {
            return Err(FilesystemError::Oversized);
        }
        let mut slot = Self::empty(handle, sensitive);
        slot.bytes[..value.len()].copy_from_slice(value);
        slot.length = value.len();
        slot.generation = 1;
        slot.exists = true;
        Ok(slot)
    }

    #[must_use]
    pub const fn exists(&self) -> bool {
        self.exists
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn sensitive(&self) -> bool {
        self.sensitive
    }
}

/// Allocator-free filesystem oracle with a finite watch queue.
pub struct MemoryFilesystem<const FILES: usize, const BYTES: usize, const EVENTS: usize> {
    slots: [FileSlot<BYTES>; FILES],
    watch: Option<WatchRequest>,
    events: [Option<WatchEvent>; EVENTS],
    event_head: usize,
    event_length: usize,
    emitted_events: usize,
    next_sequence: u64,
    gap: bool,
    cancelled: bool,
}

impl<const FILES: usize, const BYTES: usize, const EVENTS: usize>
    MemoryFilesystem<FILES, BYTES, EVENTS>
{
    #[must_use]
    pub const fn new(slots: [FileSlot<BYTES>; FILES]) -> Self {
        Self {
            slots,
            watch: None,
            events: [None; EVENTS],
            event_head: 0,
            event_length: 0,
            emitted_events: 0,
            next_sequence: 1,
            gap: false,
            cancelled: false,
        }
    }

    pub fn read(
        &self,
        request: ReadRequest,
        output: &mut [u8],
    ) -> Result<ReadResult, FilesystemError> {
        if request.maximum_bytes == 0
            || request.chunk_bytes == 0
            || request.chunk_bytes > request.maximum_bytes
            || output.len() < request.chunk_bytes
        {
            return Err(FilesystemError::InvalidBound);
        }
        let slot = self.slot(request.handle)?;
        if !slot.exists {
            return Err(FilesystemError::Missing);
        }
        let offset =
            usize::try_from(request.offset).map_err(|_| FilesystemError::OffsetOverflow)?;
        if offset >= slot.length {
            return Ok(ReadResult {
                bytes_read: 0,
                next_offset: request.offset,
                generation: slot.generation,
                eof: true,
            });
        }
        let count = request
            .chunk_bytes
            .min(request.maximum_bytes)
            .min(slot.length - offset);
        output[..count].copy_from_slice(&slot.bytes[offset..offset + count]);
        Ok(ReadResult {
            bytes_read: count,
            next_offset: request
                .offset
                .checked_add(count as u64)
                .ok_or(FilesystemError::OffsetOverflow)?,
            generation: slot.generation,
            eof: offset + count == slot.length,
        })
    }

    pub fn write(
        &mut self,
        request: WriteRequest,
        input: &[u8],
    ) -> Result<WriteResult, FilesystemError> {
        if request.maximum_bytes == 0 {
            return Err(FilesystemError::InvalidBound);
        }
        if request.requested_flush == FlushClaim::Durable {
            return Err(FilesystemError::DurabilityUnsupported);
        }
        let index = self.slot_index(request.handle)?;
        let slot = &self.slots[index];
        match request.mode {
            WriteMode::Create if slot.exists => return Err(FilesystemError::AlreadyExists),
            WriteMode::Replace | WriteMode::Append if !slot.exists => {
                return Err(FilesystemError::Missing);
            }
            WriteMode::Create | WriteMode::Replace | WriteMode::Append => {}
        }
        let start = if request.mode == WriteMode::Append {
            slot.length
        } else {
            0
        };
        let available = BYTES.saturating_sub(start);
        let accepted = input.len().min(request.maximum_bytes).min(available);
        let complete = accepted == input.len();
        if !complete && request.partial == PartialWritePolicy::FailWithoutCommit {
            return Err(
                if input.len() > request.maximum_bytes || input.len() > available {
                    FilesystemError::Oversized
                } else {
                    FilesystemError::PartialWrite
                },
            );
        }

        let slot = &mut self.slots[index];
        if request.mode != WriteMode::Append {
            slot.length = 0;
        }
        slot.bytes[start..start + accepted].copy_from_slice(&input[..accepted]);
        slot.length = start + accepted;
        slot.exists = true;
        slot.generation = slot.generation.saturating_add(1);
        let generation = slot.generation;
        let kind = if request.mode == WriteMode::Create {
            WatchEventKind::Created
        } else {
            WatchEventKind::Changed
        };
        self.push_event(kind, request.handle, generation);
        Ok(WriteResult {
            bytes_written: accepted,
            generation,
            committed: accepted > 0 || input.is_empty(),
            complete,
            flush: request.requested_flush,
        })
    }

    pub fn remove(&mut self, handle: FileHandle) -> Result<(), FilesystemError> {
        let index = self.slot_index(handle)?;
        let slot = &mut self.slots[index];
        if !slot.exists {
            return Err(FilesystemError::Missing);
        }
        slot.exists = false;
        slot.length = 0;
        slot.generation = slot.generation.saturating_add(1);
        let generation = slot.generation;
        self.push_event(WatchEventKind::Removed, handle, generation);
        Ok(())
    }

    /// Record a provider-observed rename while preserving opaque identity.
    pub fn rename(&mut self, handle: FileHandle) -> Result<(), FilesystemError> {
        let index = self.slot_index(handle)?;
        let slot = &mut self.slots[index];
        if !slot.exists {
            return Err(FilesystemError::Missing);
        }
        slot.generation = slot.generation.saturating_add(1);
        let generation = slot.generation;
        self.push_event(WatchEventKind::Renamed, handle, generation);
        Ok(())
    }

    pub fn begin_watch(&mut self, request: WatchRequest) -> Result<(), FilesystemError> {
        if request.maximum_events == 0
            || request.queue_capacity == 0
            || request.queue_capacity > EVENTS
        {
            return Err(FilesystemError::InvalidBound);
        }
        self.watch = Some(request);
        self.event_head = 0;
        self.event_length = 0;
        self.emitted_events = 0;
        self.gap = false;
        self.cancelled = false;
        if request.emit_initial {
            let mut initial = [None; FILES];
            let mut initial_length = 0;
            for slot in &self.slots {
                if slot.exists && initial_length < FILES {
                    initial[initial_length] = Some((slot.handle, slot.generation));
                    initial_length += 1;
                }
            }
            for item in initial.into_iter().flatten() {
                self.push_event(WatchEventKind::Initial, item.0, item.1);
            }
        }
        Ok(())
    }

    pub fn take_watch_event(&mut self) -> Result<Option<WatchEvent>, FilesystemError> {
        let request = self.watch.ok_or(FilesystemError::WatchInactive)?;
        if self.cancelled {
            return Err(FilesystemError::Cancelled);
        }
        if self.emitted_events >= request.maximum_events {
            return Err(FilesystemError::WatchExhausted);
        }
        if self.gap {
            self.gap = false;
            self.emitted_events += 1;
            let kind = match request.overflow {
                WatchOverflow::TerminalGap => WatchEventKind::Gap,
                WatchOverflow::GapThenResync => WatchEventKind::Resync,
            };
            return Ok(Some(self.next_event(kind, FileHandle(0), 0)));
        }
        if self.event_length == 0 {
            return Ok(None);
        }
        let event = self.events[self.event_head].take();
        self.event_head = (self.event_head + 1) % EVENTS;
        self.event_length -= 1;
        self.emitted_events += 1;
        Ok(event)
    }

    pub fn cancel_watch(&mut self) -> Result<(), FilesystemError> {
        if self.watch.is_none() {
            return Err(FilesystemError::WatchInactive);
        }
        self.cancelled = true;
        self.event_head = 0;
        self.event_length = 0;
        self.events.fill(None);
        Ok(())
    }

    fn slot(&self, handle: FileHandle) -> Result<&FileSlot<BYTES>, FilesystemError> {
        self.slots
            .iter()
            .find(|slot| slot.handle == handle)
            .ok_or(FilesystemError::WrongHandle)
    }

    fn slot_index(&self, handle: FileHandle) -> Result<usize, FilesystemError> {
        self.slots
            .iter()
            .position(|slot| slot.handle == handle)
            .ok_or(FilesystemError::WrongHandle)
    }

    fn next_event(
        &mut self,
        kind: WatchEventKind,
        handle: FileHandle,
        generation: u64,
    ) -> WatchEvent {
        let event = WatchEvent {
            sequence: self.next_sequence,
            kind,
            handle,
            generation,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        event
    }

    fn push_event(&mut self, kind: WatchEventKind, handle: FileHandle, generation: u64) {
        let Some(request) = self.watch else {
            return;
        };
        if request.coalescing == WatchCoalescing::SameHandleLatest && self.event_length > 0 {
            let tail = (self.event_head + self.event_length - 1) % EVENTS;
            if self.events[tail].is_some_and(|event| event.handle == handle) {
                self.events[tail] = Some(self.next_event(kind, handle, generation));
                return;
            }
        }
        if self.event_length >= request.queue_capacity {
            self.gap = true;
            return;
        }
        let index = (self.event_head + self.event_length) % EVENTS;
        self.events[index] = Some(self.next_event(kind, handle, generation));
        self.event_length += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Files = MemoryFilesystem<2, 8, 4>;

    fn files() -> Files {
        MemoryFilesystem::new([
            FileSlot::seeded(FileHandle(1), b"abcdef", false).unwrap(),
            FileSlot::empty(FileHandle(2), true),
        ])
    }

    #[test]
    fn bounded_range_read_reports_eof_without_path_semantics() {
        let files = files();
        let mut output = [0; 4];
        let result = files
            .read(
                ReadRequest {
                    handle: FileHandle(1),
                    offset: 2,
                    maximum_bytes: 4,
                    chunk_bytes: 4,
                    consistency: ReadConsistency::Snapshot,
                },
                &mut output,
            )
            .unwrap();
        assert_eq!(&output, b"cdef");
        assert_eq!(result.bytes_read, 4);
        assert!(result.eof);
        assert_eq!(
            files
                .read(
                    ReadRequest {
                        offset: 6,
                        ..ReadRequest {
                            handle: FileHandle(1),
                            offset: 0,
                            maximum_bytes: 4,
                            chunk_bytes: 4,
                            consistency: ReadConsistency::Snapshot,
                        }
                    },
                    &mut output,
                )
                .unwrap()
                .bytes_read,
            0
        );
    }

    #[test]
    fn create_replace_append_partial_and_durability_are_explicit() {
        let mut files = files();
        let created = files
            .write(
                WriteRequest {
                    handle: FileHandle(2),
                    mode: WriteMode::Create,
                    maximum_bytes: 4,
                    partial: PartialWritePolicy::FailWithoutCommit,
                    requested_flush: FlushClaim::ProviderAccepted,
                },
                b"new",
            )
            .unwrap();
        assert!(created.complete);
        assert_eq!(created.flush, FlushClaim::ProviderAccepted);
        assert_eq!(
            files.write(
                WriteRequest {
                    handle: FileHandle(2),
                    mode: WriteMode::Create,
                    maximum_bytes: 4,
                    partial: PartialWritePolicy::FailWithoutCommit,
                    requested_flush: FlushClaim::None,
                },
                b"x",
            ),
            Err(FilesystemError::AlreadyExists)
        );
        assert_eq!(
            files.write(
                WriteRequest {
                    handle: FileHandle(1),
                    mode: WriteMode::Replace,
                    maximum_bytes: 8,
                    partial: PartialWritePolicy::FailWithoutCommit,
                    requested_flush: FlushClaim::Durable,
                },
                b"x",
            ),
            Err(FilesystemError::DurabilityUnsupported)
        );
        let partial = files
            .write(
                WriteRequest {
                    handle: FileHandle(1),
                    mode: WriteMode::Append,
                    maximum_bytes: 8,
                    partial: PartialWritePolicy::ReportCommittedPrefix,
                    requested_flush: FlushClaim::None,
                },
                b"wxyz",
            )
            .unwrap();
        assert_eq!(partial.bytes_written, 2);
        assert!(!partial.complete);
    }

    #[test]
    fn failed_oversized_write_does_not_commit() {
        let mut files = files();
        let generation = files.slot(FileHandle(1)).unwrap().generation();
        assert_eq!(
            files.write(
                WriteRequest {
                    handle: FileHandle(1),
                    mode: WriteMode::Append,
                    maximum_bytes: 8,
                    partial: PartialWritePolicy::FailWithoutCommit,
                    requested_flush: FlushClaim::None,
                },
                b"wxyz",
            ),
            Err(FilesystemError::Oversized)
        );
        assert_eq!(files.slot(FileHandle(1)).unwrap().generation(), generation);
    }

    #[test]
    fn watch_preserves_rename_identity_and_reports_overflow() {
        let mut files = files();
        files
            .begin_watch(WatchRequest {
                emit_initial: false,
                maximum_events: 4,
                queue_capacity: 1,
                coalescing: WatchCoalescing::None,
                overflow: WatchOverflow::TerminalGap,
            })
            .unwrap();
        files.rename(FileHandle(1)).unwrap();
        files.remove(FileHandle(1)).unwrap();
        let first = files.take_watch_event().unwrap().unwrap();
        assert_eq!(first.kind, WatchEventKind::Gap);
        let renamed = files.take_watch_event().unwrap().unwrap();
        assert_eq!(renamed.kind, WatchEventKind::Renamed);
        assert_eq!(renamed.handle, FileHandle(1));
    }

    #[test]
    fn watch_initial_snapshot_resync_and_cancellation_are_finite() {
        let mut files = files();
        files
            .begin_watch(WatchRequest {
                emit_initial: true,
                maximum_events: 2,
                queue_capacity: 1,
                coalescing: WatchCoalescing::SameHandleLatest,
                overflow: WatchOverflow::GapThenResync,
            })
            .unwrap();
        assert_eq!(
            files.take_watch_event().unwrap().unwrap().kind,
            WatchEventKind::Initial
        );
        files.cancel_watch().unwrap();
        assert_eq!(files.take_watch_event(), Err(FilesystemError::Cancelled));
    }
}
