//! Optional read-only phase markers for scoped host performance measurements.
/// These mark installed execution boundaries, not semantic event time or CPU time.
/// An adapter may record native elapsed wall time; no source clock mapping follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelTimingPhase {
    PreparationStarted,
    PlayStarted,
    PlayFinished,
}
