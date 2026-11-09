//! One finite recording generation, encoded by ordinary committed kernel steps.
//! This sequence belongs to this recording stream, independently of live Face
//! publication. Context and the clock are selected by the preparing Host.
use super::*;
use alloc::{vec, vec::Vec};
use conduit_core::BoundedResourceRef;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

/// Local producer acceptance time, in milliseconds since the exact context Boot.
/// This is read while staging the observation and retained only on commit. It
/// is NOT physical acquisition time and must not retimestamp a remotely received
/// observation. The historical snapshot field is named `sampled_at_boot_ms`;
/// Source integration must explicitly select this acceptance-clock meaning,
/// or supply a different schema/input retaining actual acquisition provenance.
pub type RecordingClock = fn() -> u64;
enum Staged {
    Append { sampled_at: u64, bytes: usize },
    Emit,
}

/// No storage effects or generation rotation occur here. Output is content for
/// the separately admitted snapshot publication Back, never an acknowledgement.
pub struct RecordingEncoderBack {
    snapshot: PreparedRecordingSnapshot,
    validator: PreparedObservationQuantityPacker,
    samples: usize,
    next_sequence: u64,
    last_time: Option<u64>,
    now: RecordingClock,
    pending: Vec<u8>,
    output: Vec<u8>,
    staged: Option<Staged>,
    emitted: bool,
    cancelled: bool,
    poisoned: bool,
}
impl RecordingEncoderBack {
    /// Preparation owns snapshot prefix/output/sample storage and its pack-only
    /// codec, a second pack-only validation codec, one canonical staging buffer and
    /// one exact-extent output copy. The preparing Root must fund ALL of these;
    /// this constructor does not admit memory or storage. No runtime allocation
    /// is required by append, sealing, pressure retries or commit.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
        reference: &BoundedResourceRef,
        first_sequence: u64,
        now: RecordingClock,
    ) -> Result<Self, RecordingSnapshotRefusal> {
        if first_sequence == 0
            || first_sequence
                .checked_add(samples.saturating_sub(1) as u64)
                .is_none()
        {
            return Err(RecordingRefusal::InvalidSequence.into());
        }
        let snapshot = PreparedRecordingSnapshot::prepare(context, contract, samples, reference)?;
        let bytes = crate::canonical_storage::bytes(
            contract.observation_type(),
            crate::observation::MAXIMUM_BYTES as usize,
        )
        .map_err(RecordingRefusal::Canonical)?;
        Ok(Self {
            snapshot,
            validator: PreparedObservationQuantityPacker::prepare(contract)
                .map_err(RecordingRefusal::Canonical)?,
            samples,
            next_sequence: first_sequence,
            last_time: None,
            now,
            pending: vec![0; bytes],
            output: Vec::with_capacity(reference.extent.bytes as usize),
            staged: None,
            emitted: false,
            cancelled: false,
            poisoned: false,
        })
    }
    /// Complete owned heap capacities; excludes inline Self (including the
    /// fixed sample array), allocator overhead and shared immutable schemas.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.snapshot
            .retained_backing_bytes()?
            .checked_add(self.validator.retained_backing_bytes())?
            .checked_add(self.pending.capacity())?
            .checked_add(self.output.capacity())
    }
    pub fn committed_samples(&self) -> usize {
        self.snapshot.len()
    }
}
impl<const P: usize> StepBack<P> for RecordingEncoderBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = None;
        if self.cancelled {
            return failure_with(FailureCode::Cancelled, 1);
        }
        if self.poisoned {
            return failure_with(FailureCode::InvalidLifecycle, 2);
        }
        if self.emitted {
            return StepOutcome::Complete;
        }
        if self.snapshot.len() == self.samples {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Ok(bytes) = self.snapshot.seal() else {
                return failure_with(FailureCode::InvalidLifecycle, 3);
            };
            self.output.clear();
            self.output.extend_from_slice(bytes.as_bytes());
            if let Err(error) = io.send_prepared(PortId(0), self.output.len() as u32) {
                return io_failure(error, 4);
            }
            self.staged = Some(Staged::Emit);
            return StepOutcome::Progress;
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return failure(5);
        }
        let Some(value) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                failure(6)
            } else {
                StepOutcome::Await
            };
        };
        let Some(bytes) = input.input(PortId(0)) else {
            return failure(7);
        };
        if bytes.len() != value.byte_len as usize
            || bytes.len() > self.pending.len()
            || self.validator.pack(bytes).is_err()
        {
            return failure(8);
        }
        let sampled_at = (self.now)();
        if self.last_time.is_some_and(|last| sampled_at < last) {
            return failure(9);
        }
        if let Err(error) = io.consume_fuel(1) {
            return io_failure(error, 10);
        }
        self.pending[..bytes.len()].copy_from_slice(bytes);
        if let Err(error) = io.consume(PortId(0)) {
            return io_failure(error, 11);
        }
        self.staged = Some(Staged::Append {
            sampled_at,
            bytes: bytes.len(),
        });
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && matches!(self.staged, Some(Staged::Emit)))
            .then_some(self.output.as_slice())
    }
    fn step_committed(&mut self) {
        if self.cancelled {
            self.staged = None;
            return;
        }
        match self.staged.take() {
            Some(Staged::Append { sampled_at, bytes }) => {
                // Prevalidated same immutable Type, fresh monotonic sequence,
                // capacity and clock. No external mutation of snapshot exists.
                if self
                    .snapshot
                    .append(
                        self.next_sequence,
                        sampled_at,
                        RecordingDisposition::Available,
                        Some(&self.pending[..bytes]),
                    )
                    .is_err()
                {
                    self.poisoned = true;
                    return;
                }
                self.last_time = Some(sampled_at);
                if self.snapshot.len() < self.samples {
                    self.next_sequence += 1;
                }
            }
            Some(Staged::Emit) => self.emitted = true,
            None => {}
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
    }
}
fn failure(detail: u16) -> StepOutcome {
    failure_with(FailureCode::InvalidInput, detail)
}
fn failure_with(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code,
        detail: 2300 + detail,
    })
}
pub(super) fn io_failure(error: conduit_kernel::scheduler::SchedulerError, detail: u16) -> StepOutcome {
    use conduit_kernel::scheduler::SchedulerError as E;
    let code = match error {
        E::StepFuelExceeded => FailureCode::WorkBudgetExhausted,
        E::OutputBlocked | E::QueueCapacityExceeded | E::QueueByteCapacityExceeded => {
            FailureCode::StorageExhausted
        }
        E::InvalidPortAccess => FailureCode::InvalidPort,
        _ => FailureCode::InvalidLifecycle,
    };
    failure_with(code, detail)
}
