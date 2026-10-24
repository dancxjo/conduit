use crate::directory::{Name, SnapshotDirectory};
use conduit_core::{AuthorityBinding, BoundedResourceRef, PlannedGear};
use conduit_resource_snapshot::{PreparedSnapshotRecord, SnapshotRefusal, MAXIMUM_RECORD_BYTES};
use std::{io, os::fd::AsRawFd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoStage {
    CreateCandidate,
    WriteCandidate,
    SyncCandidate,
    PublishLink,
    RemoveCandidate,
    SyncDirectory,
    OpenRead,
    Read,
}
#[derive(Debug)]
pub enum FileSnapshotRefusal {
    Binding(SnapshotRefusal),
    InvalidKey,
    InvalidFile,
    AlreadyAttempted,
    CandidateNameExists,
    PublishedNameExists,
    Missing,
    Io {
        stage: IoStage,
        error: io::Error,
        attempt_may_have_published: bool,
    },
}
impl From<SnapshotRefusal> for FileSnapshotRefusal {
    fn from(value: SnapshotRefusal) -> Self {
        Self::Binding(value)
    }
}
fn failure(
    stage: IoStage,
    attempt_may_have_published: bool,
    error: io::Error,
) -> FileSnapshotRefusal {
    FileSnapshotRefusal::Io {
        stage,
        error,
        attempt_may_have_published,
    }
}
/// One prepared exact reference. It reserves a finite read buffer and names,
/// but Root must separately admit disk capacity, live authority and work limits.
/// No request loop, retry, thread, catalogue or Host offer is introduced here.
pub struct PreparedFileSnapshot<'a> {
    directory: &'a SnapshotDirectory,
    record: PreparedSnapshotRecord,
    published: Name,
    candidate: Name,
    buffer: [u8; MAXIMUM_RECORD_BYTES],
    attempted: bool,
}
impl<'a> PreparedFileSnapshot<'a> {
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        placement: &PlannedGear,
        reference: &BoundedResourceRef,
    ) -> Result<Self, FileSnapshotRefusal> {
        let record = PreparedSnapshotRecord::prepare(placement, reference)?;
        let published =
            Name::prepare(record.storage_key(), false).ok_or(FileSnapshotRefusal::InvalidKey)?;
        let candidate =
            Name::prepare(record.storage_key(), true).ok_or(FileSnapshotRefusal::InvalidKey)?;
        Ok(Self {
            directory,
            record,
            published,
            candidate,
            buffer: [0; MAXIMUM_RECORD_BYTES],
            attempted: false,
        })
    }
    /// Success follows file sync, non-replacing link, candidate removal and
    /// directory sync. Any attempted write is terminal for this preparation.
    /// An uncertain publication requires explicit Root reconciliation, not retry.
    pub fn publish(
        &mut self,
        authority: &AuthorityBinding,
        content: &[u8],
    ) -> Result<(), FileSnapshotRefusal> {
        if self.attempted {
            return Err(FileSnapshotRefusal::AlreadyAttempted);
        }
        let bytes = self.record.publication(authority, content)?;
        self.attempted = true;
        let candidate = self
            .directory
            .open(
                self.candidate.cstr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            )
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::EEXIST) {
                    FileSnapshotRefusal::CandidateNameExists
                } else {
                    failure(IoStage::CreateCandidate, false, e)
                }
            })?;
        // Each successful syscall consumes at least one byte. EINTR and zero
        // progress refuse rather than introducing an unbounded retry loop.
        let mut written = 0;
        while written < bytes.len() {
            // SAFETY: the immutable admitted suffix is valid for its full size.
            let count = unsafe {
                libc::write(
                    candidate.as_raw_fd(),
                    bytes[written..].as_ptr().cast(),
                    bytes.len() - written,
                )
            };
            if count <= 0 {
                return Err(failure(
                    IoStage::WriteCandidate,
                    false,
                    if count == 0 {
                        io::Error::from(io::ErrorKind::WriteZero)
                    } else {
                        io::Error::last_os_error()
                    },
                ));
            }
            written += count as usize;
        }
        candidate
            .sync_all()
            .map_err(|e| failure(IoStage::SyncCandidate, false, e))?;
        self.directory
            .link(self.candidate.cstr(), self.published.cstr())
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::EEXIST) {
                    FileSnapshotRefusal::PublishedNameExists
                } else {
                    failure(IoStage::PublishLink, true, e)
                }
            })?;
        self.directory
            .unlink(self.candidate.cstr())
            .map_err(|e| failure(IoStage::RemoveCandidate, true, e))?;
        self.directory
            .sync()
            .map_err(|e| failure(IoStage::SyncDirectory, true, e))
    }
    /// Preflight authority before opening a file. Oversized, nonregular,
    /// corrupt and symlink records never become successful content.
    pub fn read<'b>(
        &'b mut self,
        authority: &AuthorityBinding,
    ) -> Result<&'b [u8], FileSnapshotRefusal> {
        self.record.check_read_binding(authority)?;
        let file = self
            .directory
            .open(self.published.cstr(), libc::O_RDONLY)
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::ENOENT) {
                    FileSnapshotRefusal::Missing
                } else {
                    failure(IoStage::OpenRead, false, e)
                }
            })?;
        let metadata = file
            .metadata()
            .map_err(|e| failure(IoStage::Read, false, e))?;
        if !metadata.is_file() || metadata.len() > MAXIMUM_RECORD_BYTES as u64 {
            return Err(FileSnapshotRefusal::InvalidFile);
        }
        let length = metadata.len() as usize;
        let mut read = 0;
        while read < length {
            // SAFETY: the mutable admitted suffix is valid for the supplied size.
            let count = unsafe {
                libc::read(
                    file.as_raw_fd(),
                    self.buffer[read..length].as_mut_ptr().cast(),
                    length - read,
                )
            };
            if count <= 0 {
                return Err(failure(
                    IoStage::Read,
                    false,
                    if count == 0 {
                        io::Error::from(io::ErrorKind::UnexpectedEof)
                    } else {
                        io::Error::last_os_error()
                    },
                ));
            }
            read += count as usize;
        }
        let mut extra = 0_u8;
        // SAFETY: one writable byte detects growth beyond the stat'ed extent.
        let count = unsafe { libc::read(file.as_raw_fd(), (&mut extra as *mut u8).cast(), 1) };
        if count < 0 {
            return Err(failure(IoStage::Read, false, io::Error::last_os_error()));
        }
        if count != 0 {
            return Err(FileSnapshotRefusal::InvalidFile);
        }
        self.record
            .restore(authority, &self.buffer[..length])
            .map_err(Into::into)
    }
}
