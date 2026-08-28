//! Bounded normalized evidence derived from the actual OPL provider boundary.

use conduit_core::{CancellationReason, TerminalDisposition};
use conduit_semantic_catalog::{NormalizedSoundTrace, RealizedSoundEvidence, SoundEvidenceError};

use super::{
    Opl2Base, Opl2PlayReport, PreparationError, PreparedOpl2Execution, PreparedOpl2Play, cancel,
    run, validate_prepared,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opl2ConformanceReport {
    pub play: Opl2PlayReport,
    pub evidence: RealizedSoundEvidence,
}

pub fn run_with_evidence<B: Opl2Base>(
    prepared: &PreparedOpl2Play,
    execution: &mut PreparedOpl2Execution,
    base: &mut B,
) -> Result<Opl2ConformanceReport, PreparationError> {
    validate_prepared(prepared)?;
    let play = run(execution, base)?;
    let evidence = realized_evidence(execution, TerminalDisposition::Completed)
        .map_err(|_| PreparationError::KernelRejected)?;
    Ok(Opl2ConformanceReport { play, evidence })
}

pub fn cancel_with_evidence<B: Opl2Base>(
    prepared: &PreparedOpl2Play,
    execution: &mut PreparedOpl2Execution,
    base: &mut B,
    reason: CancellationReason,
) -> Result<(u16, RealizedSoundEvidence), PreparationError> {
    validate_prepared(prepared)?;
    let writes = cancel(execution, base)?;
    let evidence = realized_evidence(execution, TerminalDisposition::Cancelled { reason })
        .map_err(|_| PreparationError::KernelRejected)?;
    Ok((writes, evidence))
}

fn realized_evidence(
    execution: &mut PreparedOpl2Execution,
    terminal: TerminalDisposition,
) -> Result<RealizedSoundEvidence, SoundEvidenceError> {
    let events = core::mem::take(&mut execution.normalized_events);
    let selected = execution
        .selected
        .take()
        .ok_or(SoundEvidenceError::GateLifecycleInvalid)?;
    Ok(RealizedSoundEvidence {
        selected,
        trace: NormalizedSoundTrace::new(events, terminal)?,
    })
}
