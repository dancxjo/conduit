//! Typed worker half for one admitted Fore record read per authored input.
//! No transport is opened, authority issued, offer registered or recording ACK
//! created here. Root must supply an original guarded lease through its actual
//! native Source admission boundary. That product ownership bridge is separate.
use crate::selected_native_source::{
    NativeReadContext, NativeSourceWorkerProvider, NativeWorkerBudget, WorkerFailure,
};
use conduit_resource_snapshot::read_result::{decode_for_index, READ_RESULT_BYTES};
use std::time::Duration;

const FRAMES_PER_READ: u32 = 6;
const FRAME_BYTES: usize = conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES;

/// The caller-selected, finite lifetime ceiling; this is not an authority grant.
#[derive(Clone, Copy, Debug)]
pub struct RecordReadBudget {
    pub maximum_requests: u32,
    pub timeout_per_request: Duration,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationRefusal {
    ZeroRequests,
    FrameBudget,
    Timeout,
    WorkerProfile,
}

/// Correspondence to the retained original admitted transport, not its IDs.
///
/// # Safety
/// Root must independently verify exact original Body/Source/Plan/member Boot,
/// selected read ABI, Line/session and current authorization before each call.
/// The non-clonable lease must keep the original descriptor, approval, total
/// frame accounting and native owner alive, including after unknown outcome.
/// Dropping this worker lease must not falsely retire or release Root's original
/// unknown remote custody. Merely matching received identity strings is not
/// sufficient. Registration still uses the existing unsafe native admission API.
///
/// `exchange` must reuse the original carriage, honor the supplied deadline,
/// allocate nothing, retain no borrowed buffers, and not unwind. It must invoke
/// `consume` at most once, BEFORE its existing result transport ACK; success
/// requires actual transport delivery and that callback to have succeeded.
/// Transport backpressure is an error, not a
/// canonical record result or
/// a durable receipt. No automatic retry or new session is permitted.
pub unsafe trait OriginalRecordReadFore: Send {
    fn remaining_frames(&self) -> u32;
    fn maximum_timeout(&self) -> Duration;
    fn exchange(
        &mut self,
        input: &[u8],
        scratch: &mut [u8],
        timeout: Duration,
        consume: impl FnOnce(&[u8]) -> Result<(), ()>,
    ) -> Result<(), WorkerFailure>;
}

/// Installed through the existing finite one-slot cancellable native worker.
/// Counters and the affine lease survive child Plays; there is no reset API.
pub struct ForeRecordReadWorker<F> {
    original: F,
    budget: RecordReadBudget,
    attempted: u32,
    fenced: bool,
}
impl<F: OriginalRecordReadFore> ForeRecordReadWorker<F> {
    /// Refusal returns the original lease. Actual Root must include this finite
    /// request/frame ceiling in independent Source/admission selection, and fund
    /// ordinary worker input/output, stack and retained provider backing. The
    /// same checked worker budget must be passed to native installation.
    pub fn prepare(
        original: F,
        budget: RecordReadBudget,
        worker: NativeWorkerBudget,
    ) -> Result<Self, (F, PreparationRefusal)> {
        let refusal = if budget.maximum_requests == 0 {
            Some(PreparationRefusal::ZeroRequests)
        } else if budget
            .maximum_requests
            .checked_mul(FRAMES_PER_READ)
            .is_none_or(|frames| frames > original.remaining_frames())
        {
            Some(PreparationRefusal::FrameBudget)
        } else if budget.timeout_per_request.is_zero()
            || budget.timeout_per_request > original.maximum_timeout()
        {
            Some(PreparationRefusal::Timeout)
        } else if worker
            .backing_bytes(
                conduit_core::COUNT_ENCODED_LEN as u32,
                READ_RESULT_BYTES as u32,
            )
            .is_err()
        {
            Some(PreparationRefusal::WorkerProfile)
        } else if worker.cancellation_timeout
            < budget.timeout_per_request.min(worker.capture_timeout)
        {
            Some(PreparationRefusal::Timeout)
        } else {
            None
        };
        if let Some(reason) = refusal {
            return Err((original, reason));
        }
        Ok(Self {
            original,
            budget,
            attempted: 0,
            fenced: false,
        })
    }
    pub fn remaining_requests(&self) -> u32 {
        self.budget.maximum_requests - self.attempted
    }
    pub fn is_fenced(&self) -> bool {
        self.fenced
    }
    fn perform(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        remaining: Duration,
        stop: impl Fn() -> bool,
    ) -> Result<usize, WorkerFailure> {
        if self.fenced {
            return Err(WorkerFailure::Lost);
        }
        if stop() {
            self.fenced = true;
            return Err(WorkerFailure::Cancelled);
        }
        if self.attempted == self.budget.maximum_requests
            || self.original.remaining_frames() < FRAMES_PER_READ
        {
            self.fenced = true;
            return Err(WorkerFailure::Profile);
        }
        // Every invocation consumes one lifetime slot before validation/IO.
        // A failed or uncertain request never becomes permission for a retry.
        self.attempted += 1;
        let requested = conduit_core::decode_count(input)
            .ok()
            .and_then(|n| u32::try_from(n).ok());
        let Some(requested) = requested else {
            self.fenced = true;
            return Err(WorkerFailure::InvalidOutput);
        };
        if output.len() < READ_RESULT_BYTES || remaining.is_zero() {
            self.fenced = true;
            return Err(WorkerFailure::Profile);
        }
        let timeout = remaining.min(self.budget.timeout_per_request);
        let mut scratch = [0; FRAME_BYTES];
        let mut length = 0;
        let result = self
            .original
            .exchange(input, &mut scratch, timeout, |bytes| {
                if decode_for_index(requested, bytes).is_err()
                    || bytes.is_empty()
                    || bytes.len() > output.len()
                {
                    return Err(());
                }
                output[..bytes.len()].copy_from_slice(bytes);
                length = bytes.len();
                Ok(())
            });
        if stop() {
            self.fenced = true;
            return Err(WorkerFailure::Cancelled);
        }
        match result {
            Ok(()) if length != 0 => Ok(length),
            _ => {
                self.fenced = true;
                Err(WorkerFailure::Provider)
            }
        }
    }
}
// SAFETY: the original lease's unsafe contract supplies actual native custody
// and bounded allocation-free IO. This adapter adds finite lifetime accounting,
// canonical correlation and no retry. Root's separate registration/admission
// must establish definition correspondence and cancellation budget >= IO bound.
unsafe impl<F: OriginalRecordReadFore> NativeSourceWorkerProvider for ForeRecordReadWorker<F> {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        self.perform(input, output, context.remaining(), || context.should_stop())
    }
}
#[cfg(test)]
#[path = "fore_record_read_tests.rs"]
mod tests;
