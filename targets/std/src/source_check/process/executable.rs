//! Bounded pre-launch artifact observation. This is not hostile-filesystem
//! confinement: the selected artifact's immutability remains Root custody.
use super::{SourceCheckProcessFailure as Failure, SourceCheckProcessWorker};
use crate::RunControl;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, time::Instant};

pub(super) struct ExecutableIdentity {
    expected: [u8; 32],
    maximum_bytes: u64,
}
impl SourceCheckProcessWorker {
    /// Require an independently selected artifact digest before every launch.
    /// Configuration alone grants neither Source access nor execution authority.
    /// The Root must preserve artifact immutability through process launch; a
    /// matching observation is not an immutable executable handle or OS sandbox.
    pub fn with_executable_identity(
        mut self,
        expected: [u8; 32],
        maximum_bytes: u64,
    ) -> Result<Self, Failure> {
        if maximum_bytes == 0 || maximum_bytes > 512 * 1024 * 1024 {
            return Err(Failure::Bounds);
        }
        self.executable_identity = Some(ExecutableIdentity {
            expected,
            maximum_bytes,
        });
        Ok(self)
    }
    pub(super) fn observe_executable(
        &self,
        control: &RunControl,
        started: Instant,
    ) -> Result<(), Failure> {
        let Some(identity) = &self.executable_identity else {
            return Ok(());
        };
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&self.executable)
            .map_err(|error| Failure::ProviderLost(format!("compiler artifact: {error}")))?;
        let metadata = file.metadata().map_err(|_| Failure::ExecutableIdentity)?;
        if !metadata.is_file() || metadata.len() > identity.maximum_bytes {
            return Err(Failure::ExecutableBounds);
        }
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];
        let mut total = 0u64;
        loop {
            if control.stop_requested() {
                return Err(Failure::Cancelled);
            }
            if started.elapsed() >= self.deadline {
                return Err(Failure::Deadline);
            }
            let n = file
                .read(&mut buffer)
                .map_err(|_| Failure::ExecutableIdentity)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > identity.maximum_bytes {
                return Err(Failure::ExecutableBounds);
            }
            hasher.update(&buffer[..n]);
        }
        let observed: [u8; 32] = hasher.finalize().into();
        if observed != identity.expected {
            return Err(Failure::ExecutableIdentity);
        }
        Ok(())
    }
}
