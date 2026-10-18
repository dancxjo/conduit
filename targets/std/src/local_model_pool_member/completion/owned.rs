//! Private completion custody beyond the selected member's lifetime.
use super::LocalModelCompletedWork;
use crate::hosted_local_model::LocalModelAdapterTerminal;
use conduit_core::{MonotonicDuration, MonotonicInstant, PlannedGear, PoolSelectionEvidence};

/// An owned copy of an actual completion, with no public or deserializing
/// constructor. It retains provenance, counts and digest, not payload copies.
/// Ownership does not admit a forecast or authenticate a producer.
///
/// ```compile_fail
/// use conduit_std_host::OwnedLocalModelCompletedWork;
/// fn forge(receipt: &mut OwnedLocalModelCompletedWork) {
///     receipt.input_digest = [0; 32];
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OwnedLocalModelCompletedWork {
    selection: PoolSelectionEvidence,
    realization: PlannedGear,
    started: MonotonicInstant,
    completed: MonotonicInstant,
    elapsed: MonotonicDuration,
    input_bytes: u32,
    output_bytes: u32,
    input_digest: [u8; 32],
    terminal: LocalModelAdapterTerminal,
}

impl LocalModelCompletedWork<'_> {
    /// Copy provenance during preparation, outside execution. Callers admit
    /// storage for their bounded retained history before keeping snapshots.
    pub fn into_owned(self) -> OwnedLocalModelCompletedWork {
        OwnedLocalModelCompletedWork {
            selection: self.selection.clone(),
            realization: self.realization.clone(),
            started: self.started,
            completed: self.completed,
            elapsed: self.elapsed,
            input_bytes: self.input_bytes,
            output_bytes: self.output_bytes,
            input_digest: self.input_digest,
            terminal: self.terminal,
        }
    }
}

impl OwnedLocalModelCompletedWork {
    /// Reuse the existing completion/calibration guards over retained truth.
    /// This neither resamples clocks nor renews a source's freshness window.
    pub fn as_borrowed(&self) -> LocalModelCompletedWork<'_> {
        LocalModelCompletedWork {
            selection: &self.selection,
            realization: &self.realization,
            started: self.started.clone(),
            completed: self.completed.clone(),
            elapsed: self.elapsed,
            input_bytes: self.input_bytes,
            output_bytes: self.output_bytes,
            input_digest: self.input_digest,
            terminal: self.terminal,
        }
    }
}
