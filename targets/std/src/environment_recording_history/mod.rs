//! Original recording capture during funded IMAGE initialization, then frozen reads.
//! Reported context is data; this provider retains no storage grants or I/O.
mod capture;
mod configuration;
mod connection_limits;
mod freeze;
mod selection;
use crate::{NativeReadContext, NativeSourceWorkerProvider, WorkerFailure};
pub use capture::{prepare_capture, provider_preparation_bytes};
use conduit_core::{AuthorityGrant, BoundedResourceRef, ResourceReferenceBinding};
use conduit_linux_snapshot_store::{SnapshotDirectory, SnapshotRecordSelection};
pub use configuration::CheckedEnvironmentRecordingHistory;
pub use connection_limits::environment_recording_history_connection_limits;
use freeze::{freeze_preparation_backing_bound, freeze_snapshot};
use selection::selected_capture_preparation_backing_bound;
pub(crate) use selection::validate_capture_selection;

/// Borrowed descriptor custody plus independently retained Root authorization.
/// Construction itself establishes neither admission nor possession.
#[derive(Clone, Copy)]
pub struct CapturedSnapshotSelection<'a> {
    pub directory: &'a SnapshotDirectory,
    pub selected: SnapshotRecordSelection<'a>,
    pub reference: &'a BoundedResourceRef,
    pub original_grant: &'a AuthorityGrant,
    pub local_reference: &'a ResourceReferenceBinding,
}

pub struct PreparedEnvironmentRecordingHistorySource {
    frozen: Box<[u8]>,
}
impl PreparedEnvironmentRecordingHistorySource {
    /// Retained payload; the provider's inline Self is a separate preparation charge.
    pub fn prepared_heap_bytes(&self) -> usize {
        self.frozen.len()
    }
    fn copy_frozen(
        &self,
        input: &[u8],
        output: &mut [u8],
        stopped: Option<WorkerFailure>,
    ) -> Result<usize, WorkerFailure> {
        if let Some(reason) = stopped {
            return Err(reason);
        }
        conduit_time::decode_tick(input).map_err(|_| WorkerFailure::Provider)?;
        let target = output
            .get_mut(..self.frozen.len())
            .ok_or(WorkerFailure::InvalidOutput)?;
        target.copy_from_slice(&self.frozen);
        Ok(self.frozen.len())
    }
}
// SAFETY: preparation freezes checked canonical values and drops all IO/grants.
// Read only checks an exact Tick/deadline/cancellation and copies fixed bytes.
// There is no allocation, IO, mutation of reported contexts, or escaping borrow.
unsafe impl NativeSourceWorkerProvider for PreparedEnvironmentRecordingHistorySource {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        let stopped = if context.cancelled() {
            Some(WorkerFailure::Cancelled)
        } else if context.remaining().is_zero() {
            Some(WorkerFailure::TimedOut)
        } else {
            None
        };
        let length = self.copy_frozen(input, output, stopped)?;
        // Cancellation/deadline racing the finite copy still prevents publication.
        if context.cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        if context.remaining().is_zero() {
            return Err(WorkerFailure::TimedOut);
        }
        Ok(length)
    }
}

#[cfg(test)]
mod installed_tests;
#[cfg(test)]
mod snapshot_fixture;
#[cfg(test)]
mod tests;
