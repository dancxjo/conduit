use super::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionRole {
    Source,
    Sink,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TransferState {
    Offered(u64),
    Accepted(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SessionFailureState {
    Cancelled(u16),
    Failed(u16),
}

/// Allocation-stable lifecycle verifier for one exact directional session.
/// It owns no socket and performs no retry; lines may only submit inbound
/// and outbound frames in the order admitted here.
pub struct SessionMachine {
    binding: SessionBinding,
    role: SessionRole,
    local_hello: bool,
    peer_hello: bool,
    local_ready: bool,
    peer_ready: bool,
    transfer: Option<TransferState>,
    next_sequence: u64,
    input_closed: bool,
    local_failure: Option<SessionFailureState>,
    peer_failure: Option<SessionFailureState>,
    local_terminal: Option<SessionTerminalDisposition>,
    peer_terminal: Option<SessionTerminalDisposition>,
}

impl SessionMachine {
    pub fn new(binding: SessionBinding, role: SessionRole) -> Result<Self, WireError> {
        binding.validate()?;
        Ok(Self {
            binding,
            role,
            local_hello: false,
            peer_hello: false,
            local_ready: false,
            peer_ready: false,
            transfer: None,
            next_sequence: 0,
            input_closed: false,
            local_failure: None,
            peer_failure: None,
            local_terminal: None,
            peer_terminal: None,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn is_active(&self) -> bool {
        self.local_ready
            && self.peer_ready
            && self.local_failure.is_none()
            && self.peer_failure.is_none()
            && self.local_terminal.is_none()
            && self.peer_terminal.is_none()
    }

    pub fn is_terminal(&self) -> bool {
        self.local_terminal.is_some() && self.peer_terminal.is_some()
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn checkpoint(&self) -> SessionCheckpoint {
        SessionCheckpoint {
            next_sequence: self.next_sequence,
            transfer: match self.transfer {
                None => SessionTransferCheckpoint::None,
                Some(TransferState::Offered(sequence)) => {
                    SessionTransferCheckpoint::Offered(sequence)
                }
                Some(TransferState::Accepted(sequence)) => {
                    SessionTransferCheckpoint::Accepted(sequence)
                }
            },
            input_closed: self.input_closed,
        }
    }

    pub fn checkpoint_offer(&self) -> SessionCheckpointOffer<'_> {
        SessionCheckpointOffer {
            identity: self.binding.identity(),
            checkpoint: self.checkpoint(),
        }
    }

    /// Reconcile one finite peer checkpoint before admitting traffic on a new
    /// exact attachment for the same logical session.
    pub fn resume_with_attachment(
        &mut self,
        binding: SessionBinding,
        peer: SessionCheckpointOffer<'_>,
    ) -> Result<SessionCheckpointAcceptance, WireError> {
        binding.validate()?;
        if binding.identity() != self.binding.identity()
            || peer.identity != self.binding.identity()
            || self.local_failure.is_some()
            || self.peer_failure.is_some()
            || self.local_terminal.is_some()
            || self.peer_terminal.is_some()
        {
            return Err(WireError::InvalidSession);
        }
        let local = self.checkpoint();
        let action = reconcile_checkpoints(self.role, local, peer.checkpoint)?;
        match action {
            SessionResumeAction::Continue => {}
            SessionResumeAction::ReplayOffered(sequence) => {
                self.next_sequence = sequence;
                self.transfer = Some(TransferState::Offered(sequence));
            }
            SessionResumeAction::AwaitReplay(sequence) => {
                self.next_sequence = sequence;
                self.transfer = None;
            }
            SessionResumeAction::AdvanceDelivered(sequence) => {
                self.next_sequence = sequence.checked_add(1).ok_or(WireError::InvalidState)?;
                self.transfer = None;
            }
        }
        self.binding = binding;
        self.local_hello = false;
        self.peer_hello = false;
        self.local_ready = false;
        self.peer_ready = false;
        Ok(SessionCheckpointAcceptance {
            local,
            peer: peer.checkpoint,
            action,
            same_plan_continues: true,
        })
    }

    pub fn admit_outbound(&mut self, frame: SessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Outbound, frame)
    }

    pub fn admit_inbound(&mut self, frame: SessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Inbound, frame)
    }

    fn admit(
        &mut self,
        direction: FrameDirection,
        frame: SessionFrame<'_>,
    ) -> Result<(), WireError> {
        if !identity_matches(&self.binding, frame.identity) {
            return Err(WireError::InvalidSession);
        }
        if terminal_for(self, direction).is_some() {
            return Err(WireError::LateFrame);
        }
        match frame.message {
            SessionMessage::Hello(hello) => self.admit_hello(direction, hello),
            SessionMessage::Ready => self.admit_ready(direction),
            SessionMessage::Offered { sequence, payload } => {
                self.require_active()?;
                if self.transfer == Some(TransferState::Offered(sequence)) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.source_direction(direction)
                    || self.input_closed
                    || self.transfer.is_some()
                    || sequence != self.next_sequence
                {
                    return Err(WireError::ReorderedFrame);
                }
                if payload.len()
                    > usize::try_from(self.binding.limits.maximum_payload_bytes)
                        .map_err(|_| WireError::InvalidLimits)?
                {
                    return Err(WireError::OversizedPayload);
                }
                self.transfer = Some(TransferState::Offered(sequence));
                Ok(())
            }
            SessionMessage::Pressure { sequence } => {
                self.require_active()?;
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Offered(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.transfer = None;
                Ok(())
            }
            SessionMessage::Accepted { sequence } => {
                self.require_active()?;
                if self.transfer == Some(TransferState::Accepted(sequence)) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Offered(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.transfer = Some(TransferState::Accepted(sequence));
                Ok(())
            }
            SessionMessage::Delivered { sequence } => {
                self.require_active()?;
                if self.transfer.is_none() && sequence.checked_add(1) == Some(self.next_sequence) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Accepted(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.next_sequence = sequence.checked_add(1).ok_or(WireError::ReorderedFrame)?;
                self.transfer = None;
                Ok(())
            }
            SessionMessage::InputClosed { final_sequence } => {
                self.require_active()?;
                if self.input_closed && final_sequence == self.next_sequence {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.source_direction(direction)
                    || self.input_closed
                    || self.transfer.is_some()
                    || final_sequence != self.next_sequence
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.input_closed = true;
                Ok(())
            }
            SessionMessage::Cancelled { code } => {
                self.admit_failure(direction, SessionFailureState::Cancelled(code), code)
            }
            SessionMessage::Failed { code } => {
                self.admit_failure(direction, SessionFailureState::Failed(code), code)
            }
            SessionMessage::Terminal {
                disposition,
                final_sequence,
            } => self.admit_terminal(direction, disposition, final_sequence),
        }
    }

    fn admit_hello(
        &mut self,
        direction: FrameDirection,
        hello: SessionHello<'_>,
    ) -> Result<(), WireError> {
        if self.local_hello || self.peer_hello {
            let already_seen = match direction {
                FrameDirection::Outbound => self.local_hello,
                FrameDirection::Inbound => self.peer_hello,
            };
            if already_seen {
                return Err(WireError::DuplicateFrame);
            }
        }
        if !hello_matches(&self.binding, hello) {
            return Err(WireError::InvalidSession);
        }
        match direction {
            FrameDirection::Outbound => self.local_hello = true,
            FrameDirection::Inbound => self.peer_hello = true,
        }
        Ok(())
    }

    fn admit_ready(&mut self, direction: FrameDirection) -> Result<(), WireError> {
        if !self.local_hello || !self.peer_hello {
            return Err(WireError::InvalidState);
        }
        let ready = match direction {
            FrameDirection::Outbound => &mut self.local_ready,
            FrameDirection::Inbound => &mut self.peer_ready,
        };
        if *ready {
            return Err(WireError::DuplicateFrame);
        }
        *ready = true;
        Ok(())
    }

    fn admit_failure(
        &mut self,
        direction: FrameDirection,
        failure: SessionFailureState,
        code: u16,
    ) -> Result<(), WireError> {
        if code == 0 {
            return Err(WireError::InvalidState);
        }
        let (current, counterpart) = match direction {
            FrameDirection::Outbound => (&mut self.local_failure, self.peer_failure),
            FrameDirection::Inbound => (&mut self.peer_failure, self.local_failure),
        };
        if current.is_some() {
            return Err(WireError::DuplicateFrame);
        }
        if counterpart.is_some_and(|counterpart| counterpart != failure) {
            return Err(WireError::InvalidState);
        }
        *current = Some(failure);
        self.transfer = None;
        Ok(())
    }

    fn admit_terminal(
        &mut self,
        direction: FrameDirection,
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    ) -> Result<(), WireError> {
        if final_sequence != self.next_sequence || self.transfer.is_some() {
            return Err(WireError::ReorderedFrame);
        }
        let failure = self.local_failure.or(self.peer_failure);
        let valid = match (disposition, failure) {
            (SessionTerminalDisposition::Completed, None) => self.input_closed,
            (SessionTerminalDisposition::Cancelled, Some(SessionFailureState::Cancelled(_))) => {
                true
            }
            (SessionTerminalDisposition::Failed, Some(SessionFailureState::Failed(_))) => true,
            _ => false,
        };
        if !valid {
            return Err(WireError::InvalidState);
        }
        let peer = terminal_for(self, direction.opposite());
        if peer.is_some_and(|peer| peer != disposition) {
            return Err(WireError::InvalidState);
        }
        *terminal_for_mut(self, direction) = Some(disposition);
        Ok(())
    }

    fn require_active(&self) -> Result<(), WireError> {
        if self.is_active() {
            Ok(())
        } else {
            Err(WireError::InvalidState)
        }
    }

    fn source_direction(&self, direction: FrameDirection) -> bool {
        matches!(
            (self.role, direction),
            (SessionRole::Source, FrameDirection::Outbound)
                | (SessionRole::Sink, FrameDirection::Inbound)
        )
    }

    fn sink_direction(&self, direction: FrameDirection) -> bool {
        !self.source_direction(direction)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FrameDirection {
    Outbound,
    Inbound,
}

impl FrameDirection {
    fn opposite(self) -> Self {
        match self {
            Self::Outbound => Self::Inbound,
            Self::Inbound => Self::Outbound,
        }
    }
}

fn terminal_for(
    machine: &SessionMachine,
    direction: FrameDirection,
) -> Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => machine.local_terminal,
        FrameDirection::Inbound => machine.peer_terminal,
    }
}

fn terminal_for_mut(
    machine: &mut SessionMachine,
    direction: FrameDirection,
) -> &mut Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => &mut machine.local_terminal,
        FrameDirection::Inbound => &mut machine.peer_terminal,
    }
}
