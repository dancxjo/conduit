use conduit_core::{ExactProviderBinding, Id, ProviderObservationState, SemanticHash};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRunPhase {
    Ready,
    Running,
    Cancelling,
    Completed,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRunEvidenceKind {
    Bound,
    Started,
    Completed,
    CancellationRequested,
    Cancelled,
    ProviderLost,
    BoundExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderRunEvidence<'a> {
    pub run: Id<'a>,
    pub sequence: u32,
    pub kind: ProviderRunEvidenceKind,
    pub provider_bundle: SemanticHash,
    pub observation: SemanticHash,
    pub conformance_result: SemanticHash,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRunError {
    IllegalTransition,
    InFlightExceeded,
    ForeignQueueExceeded,
    EvidenceExceeded,
    CancellationDeadlineExceeded,
    ProviderLost,
}

/// Bounded execution witness for an already-admitted exact provider binding.
/// It owns no provider discovery, loading, process launch, or authority.
pub struct BoundedProviderRun<'a> {
    run: Id<'a>,
    binding: ExactProviderBinding<'a>,
    phase: ProviderRunPhase,
    in_flight: u16,
    foreign_queue: u16,
    evidence_events: u32,
    cancellation_deadline: Option<u64>,
}

impl<'a> BoundedProviderRun<'a> {
    #[must_use]
    pub const fn new(run: Id<'a>, binding: ExactProviderBinding<'a>) -> Self {
        Self {
            run,
            binding,
            phase: ProviderRunPhase::Ready,
            in_flight: 0,
            foreign_queue: 0,
            evidence_events: 0,
            cancellation_deadline: None,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> ProviderRunPhase {
        self.phase
    }

    pub fn start(&mut self, tick: u64) -> Result<ProviderRunEvidence<'a>, ProviderRunError> {
        if self.phase != ProviderRunPhase::Ready {
            return Err(ProviderRunError::IllegalTransition);
        }
        self.phase = ProviderRunPhase::Running;
        self.in_flight = 1;
        self.record(ProviderRunEvidenceKind::Started, tick)
    }

    pub fn set_foreign_queue(&mut self, queued: u16) -> Result<(), ProviderRunError> {
        if self.phase != ProviderRunPhase::Running {
            return Err(ProviderRunError::IllegalTransition);
        }
        if queued > self.binding.bounds.maximum_foreign_queue {
            self.phase = ProviderRunPhase::Terminal;
            return Err(ProviderRunError::ForeignQueueExceeded);
        }
        self.foreign_queue = queued;
        Ok(())
    }

    pub fn complete(&mut self, tick: u64) -> Result<ProviderRunEvidence<'a>, ProviderRunError> {
        if self.phase != ProviderRunPhase::Running {
            return Err(ProviderRunError::IllegalTransition);
        }
        self.phase = ProviderRunPhase::Completed;
        self.in_flight = 0;
        self.foreign_queue = 0;
        self.record(ProviderRunEvidenceKind::Completed, tick)
    }

    pub fn cancel(&mut self, tick: u64) -> Result<ProviderRunEvidence<'a>, ProviderRunError> {
        if self.phase != ProviderRunPhase::Running {
            return Err(ProviderRunError::IllegalTransition);
        }
        self.phase = ProviderRunPhase::Cancelling;
        self.cancellation_deadline =
            Some(tick.saturating_add(self.binding.bounds.maximum_cancellation_ticks));
        self.record(ProviderRunEvidenceKind::CancellationRequested, tick)
    }

    pub fn observe_cancelled(
        &mut self,
        tick: u64,
    ) -> Result<ProviderRunEvidence<'a>, ProviderRunError> {
        if self.phase != ProviderRunPhase::Cancelling {
            return Err(ProviderRunError::IllegalTransition);
        }
        if self
            .cancellation_deadline
            .is_some_and(|deadline| tick > deadline)
        {
            self.phase = ProviderRunPhase::Terminal;
            return Err(ProviderRunError::CancellationDeadlineExceeded);
        }
        self.phase = ProviderRunPhase::Completed;
        self.in_flight = 0;
        self.record(ProviderRunEvidenceKind::Cancelled, tick)
    }

    pub fn observe_provider_state(
        &mut self,
        state: ProviderObservationState,
        tick: u64,
    ) -> Result<Option<ProviderRunEvidence<'a>>, ProviderRunError> {
        if state == ProviderObservationState::Available {
            return Ok(None);
        }
        self.phase = ProviderRunPhase::Terminal;
        let _ = self.record(ProviderRunEvidenceKind::ProviderLost, tick)?;
        Err(ProviderRunError::ProviderLost)
    }

    fn record(
        &mut self,
        kind: ProviderRunEvidenceKind,
        tick: u64,
    ) -> Result<ProviderRunEvidence<'a>, ProviderRunError> {
        let sequence = self
            .evidence_events
            .checked_add(1)
            .ok_or(ProviderRunError::EvidenceExceeded)?;
        if sequence > self.binding.bounds.maximum_evidence_events {
            self.phase = ProviderRunPhase::Terminal;
            return Err(ProviderRunError::EvidenceExceeded);
        }
        self.evidence_events = sequence;
        Ok(ProviderRunEvidence {
            run: self.run,
            sequence,
            kind,
            provider_bundle: self.binding.provider_bundle.semantic_hash,
            observation: self.binding.observation,
            conformance_result: self.binding.conformance_result,
            tick,
        })
    }
}
