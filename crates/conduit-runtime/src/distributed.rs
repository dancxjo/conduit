//! Hosted boundary for transport-neutral distributed cords.
//!
//! Carrier implementations may allocate internally only within their exact
//! plan reservation. The trait itself requires no async runtime, executor,
//! carrier type, or allocation from callers.

use std::collections::VecDeque;

use conduit_core::{
    DistributedAuthorityContext, DistributedCordHandshake, DistributedEvidenceKind,
    DistributedHandshakeContext, DistributedReason, PlanDistributedCord, ReconnectMode,
    ResumeProof, SemanticHash, TerminalClass, validate_distributed_authority_at_use,
    validate_distributed_handshake,
};

const MAX_IN_MEMORY_FAULTS: usize = 16;

/// Readiness observed without blocking an executor thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedBackendReadiness {
    Ready,
    Pending,
    Closed,
}

/// Carrier-neutral frame category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedFrameKind {
    Value,
    Acknowledgement,
    Heartbeat,
    Cancellation,
    CancellationAcknowledgement,
    Terminal(TerminalClass),
    TerminalAcknowledgement,
}

/// Caller-borrowed outbound frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutboundDistributedFrame<'a> {
    pub kind: DistributedFrameKind,
    pub session_epoch: u64,
    pub sequence: Option<u64>,
    pub attempt: Option<u16>,
    pub correlation: Option<SemanticHash>,
    pub payload: &'a [u8],
}

/// Header returned after bytes have been copied into the caller's buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceivedDistributedFrame {
    pub kind: DistributedFrameKind,
    pub session_epoch: u64,
    pub sequence: Option<u64>,
    pub attempt: Option<u16>,
    pub correlation: Option<SemanticHash>,
    pub payload_bytes: usize,
}

/// Owned structured evidence used by hosted backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedDistributedEvidence {
    pub plan_identity: SemanticHash,
    pub binding_identity: SemanticHash,
    pub cord: String,
    pub session: String,
    pub session_epoch: u64,
    pub sequence: Option<u64>,
    pub attempt: Option<u16>,
    pub correlation: Option<SemanticHash>,
    pub kind: DistributedEvidenceKind,
    pub reason: Option<DistributedReason>,
}

/// Host-implemented distributed-cord boundary.
///
/// An implementation receives an already resolved exact binding. It does not
/// discover peers, select a carrier, acquire a grant, provision a host, or
/// reinterpret delivery semantics.
pub trait DistributedCordBackend {
    type Error;
    type Evidence;

    fn open(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error>;

    /// Revalidate live peer proofs after a partition. Same-epoch resume also
    /// carries the bounded session-machine proof; a new epoch is explicit.
    fn reauthenticate(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        resume: Option<ResumeProof>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error>;

    fn send_readiness(&self) -> DistributedBackendReadiness;

    fn send(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        frame: OutboundDistributedFrame<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error>;

    fn receive_readiness(&self) -> DistributedBackendReadiness;

    fn receive(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        destination: &mut [u8],
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<Option<ReceivedDistributedFrame>, Self::Error>;

    fn cancel(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        session_epoch: u64,
        sequence: u64,
        correlation: Option<SemanticHash>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        self.send(
            binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::Cancellation,
                session_epoch,
                sequence: Some(sequence),
                attempt: None,
                correlation,
                payload: &[],
            },
            authority,
        )
    }

    fn close(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        session_epoch: u64,
        sequence: u64,
        terminal: TerminalClass,
        correlation: Option<SemanticHash>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        self.send(
            binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::Terminal(terminal),
                session_epoch,
                sequence: Some(sequence),
                attempt: None,
                correlation,
                payload: &[],
            },
            authority,
        )
    }

    fn take_evidence(&mut self) -> Option<Self::Evidence>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedFrame {
    kind: DistributedFrameKind,
    session_epoch: u64,
    sequence: Option<u64>,
    attempt: Option<u16>,
    correlation: Option<SemanticHash>,
    payload: Vec<u8>,
}

impl OwnedFrame {
    fn borrowed(&self) -> OutboundDistributedFrame<'_> {
        OutboundDistributedFrame {
            kind: self.kind,
            session_epoch: self.session_epoch,
            sequence: self.sequence,
            attempt: self.attempt,
            correlation: self.correlation,
            payload: &self.payload,
        }
    }
}

/// Deterministic next-frame fault for conformance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InMemoryTransportFault {
    DropNextAcknowledgement,
    DuplicateNextValue,
    ReorderNextValuePair,
}

/// Bounded in-memory backend used to exercise carrier faults without giving a
/// carrier library semantic authority.
#[derive(Debug)]
pub struct InMemoryDistributedCordBackend {
    open: bool,
    partitioned: bool,
    plan_identity: SemanticHash,
    binding_identity: SemanticHash,
    cord: String,
    session: String,
    session_epoch: u64,
    maximum_payload_bytes: u32,
    maximum_frame_bytes: u32,
    maximum_receive_items: u16,
    maximum_receive_bytes: u64,
    maximum_evidence_events: u16,
    queued_bytes: u64,
    frames: VecDeque<OwnedFrame>,
    held_reorder: Option<OwnedFrame>,
    faults: VecDeque<InMemoryTransportFault>,
    evidence: VecDeque<HostedDistributedEvidence>,
}

impl Default for InMemoryDistributedCordBackend {
    fn default() -> Self {
        Self {
            open: false,
            partitioned: false,
            plan_identity: SemanticHash::from_bytes([0; 32]),
            binding_identity: SemanticHash::from_bytes([0; 32]),
            cord: String::new(),
            session: String::new(),
            session_epoch: 0,
            maximum_payload_bytes: 0,
            maximum_frame_bytes: 0,
            maximum_receive_items: 0,
            maximum_receive_bytes: 0,
            maximum_evidence_events: 0,
            queued_bytes: 0,
            frames: VecDeque::new(),
            held_reorder: None,
            faults: VecDeque::new(),
            evidence: VecDeque::new(),
        }
    }
}

impl InMemoryDistributedCordBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inject_fault(&mut self, fault: InMemoryTransportFault) -> Result<(), DistributedReason> {
        if self.faults.len() >= MAX_IN_MEMORY_FAULTS {
            return Err(DistributedReason::BufferFull);
        }
        self.faults.push_back(fault);
        Ok(())
    }

    pub fn set_partitioned(&mut self, partitioned: bool) {
        self.partitioned = partitioned;
    }

    #[must_use]
    pub fn queued_items(&self) -> usize {
        self.frames.len() + usize::from(self.held_reorder.is_some())
    }

    #[must_use]
    pub const fn queued_bytes(&self) -> u64 {
        self.queued_bytes
    }

    fn evidence_for(
        &self,
        frame: Option<OutboundDistributedFrame<'_>>,
        kind: DistributedEvidenceKind,
        reason: Option<DistributedReason>,
    ) -> HostedDistributedEvidence {
        HostedDistributedEvidence {
            plan_identity: self.plan_identity,
            binding_identity: self.binding_identity,
            cord: self.cord.clone(),
            session: self.session.clone(),
            session_epoch: frame.map_or(self.session_epoch, |frame| frame.session_epoch),
            sequence: frame.and_then(|frame| frame.sequence),
            attempt: frame.and_then(|frame| frame.attempt),
            correlation: frame.and_then(|frame| frame.correlation),
            kind,
            reason,
        }
    }

    fn push_evidence(
        &mut self,
        evidence: HostedDistributedEvidence,
    ) -> Result<(), DistributedReason> {
        self.ensure_evidence_capacity()?;
        self.evidence.push_back(evidence);
        Ok(())
    }

    fn reject_frame<T>(
        &mut self,
        frame: OutboundDistributedFrame<'_>,
        reason: DistributedReason,
    ) -> Result<T, DistributedReason> {
        self.push_evidence(self.evidence_for(
            Some(frame),
            DistributedEvidenceKind::FrameRejected,
            Some(reason),
        ))?;
        Err(reason)
    }

    fn ensure_evidence_capacity(&self) -> Result<(), DistributedReason> {
        if self.evidence.len() >= usize::from(self.maximum_evidence_events) {
            Err(DistributedReason::EvidenceFull)
        } else {
            Ok(())
        }
    }

    fn ensure_capacity(
        &self,
        frame: &OwnedFrame,
        additional: u16,
    ) -> Result<(), DistributedReason> {
        let items = u16::try_from(self.queued_items())
            .ok()
            .and_then(|items| items.checked_add(additional))
            .ok_or(DistributedReason::BufferFull)?;
        let additional_bytes = u64::try_from(frame.payload.len())
            .ok()
            .and_then(|bytes| bytes.checked_mul(u64::from(additional)))
            .ok_or(DistributedReason::BufferFull)?;
        let bytes = self
            .queued_bytes
            .checked_add(additional_bytes)
            .ok_or(DistributedReason::BufferFull)?;
        if items > self.maximum_receive_items || bytes > self.maximum_receive_bytes {
            return Err(DistributedReason::BufferFull);
        }
        Ok(())
    }

    fn push_frame(&mut self, frame: OwnedFrame) {
        self.queued_bytes += u64::try_from(frame.payload.len()).unwrap_or(u64::MAX);
        self.frames.push_back(frame);
    }
}

impl DistributedCordBackend for InMemoryDistributedCordBackend {
    type Error = DistributedReason;
    type Evidence = HostedDistributedEvidence;

    fn open(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        validate_distributed_handshake(binding, handshake, context)?;
        validate_distributed_authority_at_use(binding, authority)?;
        if self.open || handshake.session_epoch != binding.initial_session_epoch {
            return Err(DistributedReason::HandshakeMismatch);
        }
        self.plan_identity = handshake.plan_identity;
        self.binding_identity = binding.identity;
        self.cord = binding.cord.as_str().to_owned();
        self.session = binding.session.as_str().to_owned();
        self.session_epoch = handshake.session_epoch;
        self.maximum_payload_bytes = binding.budget.maximum_payload_bytes;
        self.maximum_frame_bytes = binding.budget.maximum_frame_bytes;
        self.maximum_receive_items = binding.budget.receive_items;
        self.maximum_receive_bytes = binding.budget.receive_bytes;
        self.maximum_evidence_events = binding.budget.maximum_evidence_events;
        self.open = true;
        self.push_evidence(self.evidence_for(
            None,
            DistributedEvidenceKind::HandshakeAccepted,
            None,
        ))
    }

    fn reauthenticate(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        resume: Option<ResumeProof>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        validate_distributed_handshake(binding, handshake, context)?;
        validate_distributed_authority_at_use(binding, authority)?;
        if !self.open
            || handshake.plan_identity != self.plan_identity
            || handshake.binding_identity != self.binding_identity
        {
            return Err(DistributedReason::HandshakeMismatch);
        }
        match binding.reconnect {
            ReconnectMode::Reject => return Err(DistributedReason::ReconnectDenied),
            ReconnectMode::ResumeSameEpoch => {
                let proof = resume.ok_or(DistributedReason::ReconnectDenied)?;
                if handshake.session_epoch != self.session_epoch
                    || proof.plan_identity != self.plan_identity
                    || proof.binding_identity != self.binding_identity
                    || proof.session_epoch != self.session_epoch
                    || proof.receipt == SemanticHash::from_bytes([0; 32])
                {
                    return Err(DistributedReason::EpochMismatch);
                }
            }
            ReconnectMode::BeginNewEpoch => {
                if resume.is_some()
                    || self.session_epoch.checked_add(1) != Some(handshake.session_epoch)
                {
                    return Err(DistributedReason::EpochMismatch);
                }
            }
        }
        self.ensure_evidence_capacity()?;
        self.session_epoch = handshake.session_epoch;
        self.partitioned = false;
        self.push_evidence(self.evidence_for(None, DistributedEvidenceKind::Reconnected, None))
    }

    fn send_readiness(&self) -> DistributedBackendReadiness {
        if !self.open {
            DistributedBackendReadiness::Closed
        } else if self.partitioned
            || self.queued_items() >= usize::from(self.maximum_receive_items)
            || self.queued_bytes >= self.maximum_receive_bytes
        {
            DistributedBackendReadiness::Pending
        } else {
            DistributedBackendReadiness::Ready
        }
    }

    fn send(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        frame: OutboundDistributedFrame<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        if binding.identity != self.binding_identity {
            return self.reject_frame(frame, DistributedReason::IdentityMismatch);
        }
        if let Err(reason) = validate_distributed_authority_at_use(binding, authority) {
            return self.reject_frame(frame, reason);
        }
        if !self.open || frame.session_epoch != self.session_epoch {
            return self.reject_frame(frame, DistributedReason::EpochMismatch);
        }
        self.ensure_evidence_capacity()?;
        if self.partitioned {
            self.push_evidence(self.evidence_for(
                Some(frame),
                DistributedEvidenceKind::Disconnected,
                Some(DistributedReason::Partitioned),
            ))?;
            return Err(DistributedReason::Partitioned);
        }
        let payload_bytes =
            u32::try_from(frame.payload.len()).map_err(|_| DistributedReason::OversizedFrame);
        let payload_bytes = match payload_bytes {
            Ok(payload_bytes) => payload_bytes,
            Err(reason) => return self.reject_frame(frame, reason),
        };
        if payload_bytes > self.maximum_payload_bytes || payload_bytes > self.maximum_frame_bytes {
            return self.reject_frame(frame, DistributedReason::OversizedFrame);
        }
        let owned = OwnedFrame {
            kind: frame.kind,
            session_epoch: frame.session_epoch,
            sequence: frame.sequence,
            attempt: frame.attempt,
            correlation: frame.correlation,
            payload: frame.payload.to_vec(),
        };
        let fault = self.faults.front().copied();
        if fault == Some(InMemoryTransportFault::DropNextAcknowledgement)
            && frame.kind == DistributedFrameKind::Acknowledgement
        {
            self.faults.pop_front();
            self.push_evidence(self.evidence_for(
                Some(frame),
                DistributedEvidenceKind::FrameDropped,
                None,
            ))?;
            return Ok(());
        }
        if fault == Some(InMemoryTransportFault::DuplicateNextValue)
            && frame.kind == DistributedFrameKind::Value
        {
            if let Err(reason) = self.ensure_capacity(&owned, 2) {
                return self.reject_frame(frame, reason);
            }
            self.faults.pop_front();
            self.push_frame(owned.clone());
            self.push_frame(owned);
        } else if fault == Some(InMemoryTransportFault::ReorderNextValuePair)
            && frame.kind == DistributedFrameKind::Value
        {
            if let Err(reason) = self.ensure_capacity(&owned, 1) {
                return self.reject_frame(frame, reason);
            }
            if let Some(held) = self.held_reorder.take() {
                self.faults.pop_front();
                self.push_frame(owned);
                self.frames.push_back(held);
            } else {
                self.queued_bytes += u64::try_from(owned.payload.len()).unwrap_or(u64::MAX);
                self.held_reorder = Some(owned);
            }
        } else {
            if let Err(reason) = self.ensure_capacity(&owned, 1) {
                return self.reject_frame(frame, reason);
            }
            self.push_frame(owned);
        }
        let kind = match frame.kind {
            DistributedFrameKind::Value if frame.attempt.is_some() => {
                DistributedEvidenceKind::Retried
            }
            DistributedFrameKind::Value => DistributedEvidenceKind::ValueSent,
            DistributedFrameKind::Acknowledgement => DistributedEvidenceKind::Acknowledged,
            DistributedFrameKind::Cancellation
            | DistributedFrameKind::CancellationAcknowledgement => {
                DistributedEvidenceKind::Cancelled
            }
            DistributedFrameKind::Terminal(_) | DistributedFrameKind::TerminalAcknowledgement => {
                DistributedEvidenceKind::Terminal
            }
            DistributedFrameKind::Heartbeat => DistributedEvidenceKind::Heartbeat,
        };
        self.push_evidence(self.evidence_for(Some(frame), kind, None))
    }

    fn receive_readiness(&self) -> DistributedBackendReadiness {
        if !self.open {
            DistributedBackendReadiness::Closed
        } else if self.frames.is_empty() {
            DistributedBackendReadiness::Pending
        } else {
            DistributedBackendReadiness::Ready
        }
    }

    fn receive(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        destination: &mut [u8],
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<Option<ReceivedDistributedFrame>, Self::Error> {
        if binding.identity != self.binding_identity {
            return Err(DistributedReason::IdentityMismatch);
        }
        if let Err(reason) = validate_distributed_authority_at_use(binding, authority) {
            if let Some(frame) = self.frames.front() {
                let frame = OutboundDistributedFrame {
                    kind: frame.kind,
                    session_epoch: frame.session_epoch,
                    sequence: frame.sequence,
                    attempt: frame.attempt,
                    correlation: frame.correlation,
                    payload: &[],
                };
                return self.reject_frame(frame, reason);
            }
            return Err(reason);
        }
        let Some(frame) = self.frames.front() else {
            return Ok(None);
        };
        if destination.len() < frame.payload.len() {
            let frame = OutboundDistributedFrame {
                kind: frame.kind,
                session_epoch: frame.session_epoch,
                sequence: frame.sequence,
                attempt: frame.attempt,
                correlation: frame.correlation,
                payload: &[],
            };
            return self.reject_frame(frame, DistributedReason::BufferFull);
        }
        self.ensure_evidence_capacity()?;
        let frame = self.frames.pop_front().expect("front was present");
        destination[..frame.payload.len()].copy_from_slice(&frame.payload);
        self.queued_bytes -= u64::try_from(frame.payload.len()).unwrap_or(0);
        let borrowed = frame.borrowed();
        self.push_evidence(self.evidence_for(
            Some(borrowed),
            DistributedEvidenceKind::ValueReceived,
            None,
        ))?;
        Ok(Some(ReceivedDistributedFrame {
            kind: frame.kind,
            session_epoch: frame.session_epoch,
            sequence: frame.sequence,
            attempt: frame.attempt,
            correlation: frame.correlation,
            payload_bytes: frame.payload.len(),
        }))
    }

    fn take_evidence(&mut self) -> Option<Self::Evidence> {
        self.evidence.pop_front()
    }
}
