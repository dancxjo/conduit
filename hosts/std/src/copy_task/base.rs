use super::model::CopyResult;
use conduit_core::ProtectedResourceCommitPolicy;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ExecutionFaults {
    pub(super) fail_after_bytes: Option<u64>,
    pub(super) stop_after_bytes: Option<u64>,
    pub(super) cleanup_failure: bool,
}

pub(super) struct CopyFiles {
    source: File,
    temporary: Option<File>,
    temporary_path: PathBuf,
    destination_path: PathBuf,
    policy: ProtectedResourceCommitPolicy,
    maximum_bytes: u64,
    pub(super) bytes_copied: u64,
    pub(super) faults: ExecutionFaults,
}

impl CopyFiles {
    pub(super) fn prepare(
        source_path: &Path,
        destination_path: &Path,
        policy: ProtectedResourceCommitPolicy,
        maximum_bytes: u64,
        faults: ExecutionFaults,
    ) -> Result<Self, CopyResult> {
        let source = File::open(source_path).map_err(source_open_result)?;
        let temporary_path = temporary_path(destination_path);
        let temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(temporary_open_result)?;
        Ok(Self {
            source,
            temporary: Some(temporary),
            temporary_path,
            destination_path: destination_path.to_path_buf(),
            policy,
            maximum_bytes,
            bytes_copied: 0,
            faults,
        })
    }

    pub(super) fn step(&mut self) -> Result<bool, CopyResult> {
        if self
            .faults
            .fail_after_bytes
            .is_some_and(|limit| self.bytes_copied >= limit)
        {
            return Err(self.fail_and_cleanup());
        }
        let mut chunk = [0_u8; conduit_std_catalog::COPY_CHUNK_BYTES as usize];
        let read = self
            .source
            .read(&mut chunk)
            .map_err(|_| self.fail_and_cleanup())?;
        if read != 0 {
            let next_bytes = self
                .bytes_copied
                .checked_add(read as u64)
                .ok_or_else(|| self.fail_and_cleanup())?;
            if next_bytes > self.maximum_bytes {
                let result = if self.cleanup() {
                    CopyResult::Oversized {
                        source_bytes: next_bytes,
                        maximum_bytes: self.maximum_bytes,
                    }
                } else {
                    CopyResult::CleanupFailed {
                        bytes_copied: self.bytes_copied,
                    }
                };
                return Err(result);
            }
            let Some(temporary) = self.temporary.as_mut() else {
                return Err(self.fail_and_cleanup());
            };
            if temporary.write_all(&chunk[..read]).is_err() {
                return Err(self.fail_and_cleanup());
            }
            self.bytes_copied = next_bytes;
            return Ok(true);
        }
        self.commit()?;
        Ok(false)
    }

    pub(super) fn cleanup(&mut self) -> bool {
        self.temporary.take();
        if self.faults.cleanup_failure {
            return false;
        }
        match std::fs::remove_file(&self.temporary_path) {
            Ok(()) => true,
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        }
    }

    fn commit(&mut self) -> Result<(), CopyResult> {
        let temporary = self
            .temporary
            .take()
            .ok_or_else(|| self.fail_and_cleanup())?;
        temporary.sync_all().map_err(|_| self.fail_and_cleanup())?;
        drop(temporary);
        let commit = match self.policy {
            ProtectedResourceCommitPolicy::CreateOnly => {
                match std::fs::hard_link(&self.temporary_path, &self.destination_path) {
                    Ok(()) => {
                        return std::fs::remove_file(&self.temporary_path).map_err(|_| {
                            CopyResult::CleanupFailed {
                                bytes_copied: self.bytes_copied,
                            }
                        });
                    }
                    Err(error) => Err(error),
                }
            }
            ProtectedResourceCommitPolicy::ReplaceExisting => {
                std::fs::rename(&self.temporary_path, &self.destination_path)
            }
            ProtectedResourceCommitPolicy::NotApplicable => return Err(self.fail_and_cleanup()),
        };
        commit.map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                let _ = self.cleanup();
                CopyResult::DestinationExists
            } else {
                self.fail_and_cleanup()
            }
        })
    }

    fn fail_and_cleanup(&mut self) -> CopyResult {
        if self.cleanup() {
            CopyResult::Partial {
                bytes_copied: self.bytes_copied,
            }
        } else {
            CopyResult::CleanupFailed {
                bytes_copied: self.bytes_copied,
            }
        }
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "destination".into());
    name.push(".conduit-copy.tmp");
    destination.with_file_name(name)
}

fn source_open_result(error: std::io::Error) -> CopyResult {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        CopyResult::Denied
    } else {
        CopyResult::StaleHandle
    }
}

fn temporary_open_result(error: std::io::Error) -> CopyResult {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => CopyResult::Denied,
        std::io::ErrorKind::AlreadyExists => CopyResult::CleanupFailed { bytes_copied: 0 },
        _ => CopyResult::Partial { bytes_copied: 0 },
    }
}
