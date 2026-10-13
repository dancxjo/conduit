//! One fixed completion slot admitted with the selected member. Sampling and
//! hashing allocate no storage during execution; typed projection happens later.
//! This measures elapsed adapter-call time, including effects it waits for. It
//! does not separately measure queue/transport/warmup, predict completion, assert
//! residency, or grant authority to publish a cost observation.
mod calibration;
mod owned;
pub use calibration::{LocalModelCompletionCalibrationRefusal, LocalModelPreparedCalibration};
pub use owned::OwnedLocalModelCompletedWork;

use crate::{hosted_local_model::LocalModelAdapterTerminal, ThreadTimer, TimerAdapter};
use conduit_core::{
    MonotonicClockIdentity, MonotonicDuration, MonotonicInstant, MonotonicTimeRefusal, PlannedGear,
    PoolSelectionEvidence,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LocalModelMeasurementRefusal {
    NotCompleted,
    ClockUnavailable,
    MissingBounds,
    InputExtent,
    OutputExtent,
    Clock(MonotonicTimeRefusal),
}

/// Borrows the actual sealed selection and placement; callers cannot substitute
/// another model, Host, Boot, offer generation, operation or realization.
///
/// ```
/// use conduit_std_host::{LocalModelCompletedWork, LocalModelPreparedCalibration};
/// fn read(receipt: &LocalModelCompletedWork<'_>) -> [u8; 32] {
///     receipt.input_digest()
/// }
/// fn record<'a>(prepared: &LocalModelPreparedCalibration<'a>) {
///     let _ = prepared.record().measured();
/// }
/// ```
///
/// ```compile_fail
/// use conduit_std_host::LocalModelCompletedWork;
/// fn forge(receipt: &mut LocalModelCompletedWork<'_>) {
///     receipt.input_digest = [0; 32];
/// }
/// ```
#[derive(Debug)]
pub struct LocalModelCompletedWork<'a> {
    selection: &'a PoolSelectionEvidence,
    realization: &'a PlannedGear,
    started: MonotonicInstant,
    completed: MonotonicInstant,
    elapsed: MonotonicDuration,
    input_bytes: u32,
    output_bytes: u32,
    input_digest: [u8; 32],
    terminal: LocalModelAdapterTerminal,
}

impl<'a> LocalModelCompletedWork<'a> {
    pub fn selection(&self) -> &'a PoolSelectionEvidence {
        self.selection
    }
    pub fn realization(&self) -> &'a PlannedGear {
        self.realization
    }
    pub fn started(&self) -> &MonotonicInstant {
        &self.started
    }
    pub fn completed(&self) -> &MonotonicInstant {
        &self.completed
    }
    pub const fn elapsed(&self) -> MonotonicDuration {
        self.elapsed
    }
    pub const fn input_bytes(&self) -> u32 {
        self.input_bytes
    }
    pub const fn output_bytes(&self) -> u32 {
        self.output_bytes
    }
    pub const fn input_digest(&self) -> [u8; 32] {
        self.input_digest
    }
    pub const fn terminal(&self) -> LocalModelAdapterTerminal {
        self.terminal
    }
}

pub(super) struct MeasurementSlot {
    clock: Option<MonotonicClockIdentity>,
    bounds: Option<(u32, u32)>,
    started: Option<u64>,
    completed: Option<u64>,
    input_bytes: u32,
    output_bytes: u32,
    input_digest: [u8; 32],
    terminal: Option<LocalModelAdapterTerminal>,
    refusal: Option<LocalModelMeasurementRefusal>,
}
impl MeasurementSlot {
    pub(super) fn admit(placement: &PlannedGear) -> Self {
        // Prepare the clock identity and process epoch before adapter execution.
        // Later counter reads use this same ThreadTimer epoch and allocate no IDs.
        let clock = ThreadTimer
            .monotonic_observation(&placement.host_id, &placement.boot_id)
            .map(|sample| sample.clock().clone());
        let mut calls = placement.host_calls.iter().filter(|call| {
            call.contract_id.as_str() == conduit_ai::LOCAL_MODEL_OPERATION
                && call.target_kind.as_ref() == Some(&placement.kind_id)
        });
        let bounds = calls.next().and_then(|call| {
            (calls.next().is_none())
                .then_some((call.maximum_input_bytes, call.maximum_output_bytes))
        });
        Self {
            clock,
            bounds,
            started: None,
            completed: None,
            input_bytes: 0,
            output_bytes: 0,
            input_digest: [0; 32],
            terminal: None,
            refusal: None,
        }
    }
    pub(super) fn start(&mut self, input: &[u8]) {
        let Some((maximum, _)) = self.bounds else {
            self.refusal = Some(LocalModelMeasurementRefusal::MissingBounds);
            return;
        };
        let Ok(bytes) = u32::try_from(input.len()) else {
            self.refusal = Some(LocalModelMeasurementRefusal::InputExtent);
            return;
        };
        if bytes > maximum {
            self.refusal = Some(LocalModelMeasurementRefusal::InputExtent);
            return;
        }
        self.input_bytes = bytes;
        // Exact encoded request custody, not retained prompt text or an owner-
        // selected workload label. Hashing precedes the timed adapter interval.
        self.input_digest =
            conduit_core::semantic_digest("conduit.std/local-model-completed-input@1", input);
        self.started = ThreadTimer.monotonic_now_micros();
    }
    pub(super) fn input_identity(&self) -> Option<(u32, [u8; 32])> {
        self.refusal
            .is_none()
            .then_some((self.input_bytes, self.input_digest))
    }
    pub(super) fn finish(&mut self, terminal: LocalModelAdapterTerminal, bytes: usize) {
        self.completed = ThreadTimer.monotonic_now_micros();
        self.terminal = Some(terminal);
        if self.refusal.is_some() {
            return;
        }
        let (_, maximum) = self.bounds.expect("start admitted exact bounds");
        match u32::try_from(bytes) {
            Ok(bytes) if bytes <= maximum => self.output_bytes = bytes,
            _ => self.refusal = Some(LocalModelMeasurementRefusal::OutputExtent),
        }
    }
    pub(super) fn prepare<'a>(
        &self,
        realization: &'a PlannedGear,
        selection: &'a PoolSelectionEvidence,
    ) -> Result<LocalModelCompletedWork<'a>, LocalModelMeasurementRefusal> {
        let (started, completed, elapsed, terminal) = self.prepare_times()?;
        Ok(LocalModelCompletedWork {
            selection,
            realization,
            started,
            completed,
            elapsed,
            input_bytes: self.input_bytes,
            output_bytes: self.output_bytes,
            input_digest: self.input_digest,
            terminal,
        })
    }
    fn prepare_times(
        &self,
    ) -> Result<
        (
            MonotonicInstant,
            MonotonicInstant,
            MonotonicDuration,
            LocalModelAdapterTerminal,
        ),
        LocalModelMeasurementRefusal,
    > {
        use LocalModelMeasurementRefusal as R;
        let terminal = self.terminal.ok_or(R::NotCompleted)?;
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        let clock = self.clock.as_ref().ok_or(R::ClockUnavailable)?;
        let started =
            MonotonicInstant::new(self.started.ok_or(R::ClockUnavailable)?, clock.clone())
                .map_err(R::Clock)?;
        let completed =
            MonotonicInstant::new(self.completed.ok_or(R::ClockUnavailable)?, clock.clone())
                .map_err(R::Clock)?;
        let elapsed = completed.elapsed_since(&started).map_err(R::Clock)?;
        Ok((started, completed, elapsed, terminal))
    }
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
