//! Shared finite delivery lifecycle, independent of identity envelope.
use super::*;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::session) enum TransferState {
    Offered(u64),
    Accepted(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::session) enum SessionFailureState {
    Cancelled(u16),
    Failed(u16),
}

/// Allocation-stable lifecycle verifier for one exact directional session.
/// It owns no socket and performs no retry; lines may only submit inbound
/// and outbound frames in the order admitted here.
#[derive(Clone)]
pub struct SessionMachine<B = SessionBinding> {
    pub(in crate::session) binding: B,
    pub(in crate::session) role: SessionRole,
    pub(in crate::session) local_hello: bool,
    pub(in crate::session) peer_hello: bool,
    pub(in crate::session) local_ready: bool,
    pub(in crate::session) peer_ready: bool,
    pub(in crate::session) transfer: Option<TransferState>,
    pub(in crate::session) next_sequence: u64,
    pub(in crate::session) input_closed: bool,
    pub(in crate::session) input_abnormal: bool,
    pub(in crate::session) abnormal_terminal_digest: Option<[u8; 32]>,
    pub(in crate::session) replay_input_terminal: bool,
    pub(in crate::session) local_failure: Option<SessionFailureState>,
    pub(in crate::session) peer_failure: Option<SessionFailureState>,
    pub(in crate::session) local_terminal: Option<SessionTerminalDisposition>,
    pub(in crate::session) peer_terminal: Option<SessionTerminalDisposition>,
}

pub(in crate::session) trait Scope {
    fn limits(&self) -> SessionLimits;
    fn abnormal_kind(&self) -> Option<&conduit_core::KindId>;
}
impl Scope for SessionBinding {
    fn limits(&self) -> SessionLimits {
        self.limits
    }
    fn abnormal_kind(&self) -> Option<&conduit_core::KindId> {
        self.abnormal_kind.as_ref()
    }
}
#[allow(private_bounds)] // Scope is sealed to the two verified identity envelopes.
impl<B: Scope> SessionMachine<B> {
    pub(in crate::session) fn with_scope(binding: B, role: SessionRole) -> Self {
        Self {
            binding,
            role,
            local_hello: false,
            peer_hello: false,
            local_ready: false,
            peer_ready: false,
            transfer: None,
            next_sequence: 0,
            input_closed: false,
            input_abnormal: false,
            abnormal_terminal_digest: None,
            replay_input_terminal: false,
            local_failure: None,
            peer_failure: None,
            local_terminal: None,
            peer_terminal: None,
        }
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
                    SessionTransferCheckpoint::offered(sequence)
                        .expect("u64 transfer sequences are exact")
                }
                Some(TransferState::Accepted(sequence)) => {
                    SessionTransferCheckpoint::accepted(sequence)
                        .expect("u64 transfer sequences are exact")
                }
            },
            input_closed: self.input_closed,
            input_abnormal: self.input_abnormal,
            abnormal_terminal_digest: self.abnormal_terminal_digest,
        }
    }

    pub(in crate::session) fn admit_message(
        &mut self,
        direction: FrameDirection,
        message: SessionMessage<'_>,
    ) -> Result<(), WireError> {
        if terminal_for(self, direction).is_some() {
            return Err(WireError::LateFrame);
        }
        match message {
            SessionMessage::Hello(_) => Err(WireError::InvalidMessageKind),
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
                    > usize::try_from(self.binding.limits().maximum_payload_bytes)
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
                if self.source_direction(direction) && self.replay_input_terminal {
                    if self.input_closed
                        && !self.input_abnormal
                        && final_sequence == self.next_sequence
                    {
                        self.replay_input_terminal = false;
                        return Ok(());
                    }
                    return Err(WireError::ValueContractMismatch);
                }
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
                self.input_abnormal = false;
                self.abnormal_terminal_digest = None;
                Ok(())
            }
            SessionMessage::InputAbnormal {
                final_sequence,
                terminal,
            } => {
                self.require_active()?;
                let Some(abnormal_kind) = self.binding.abnormal_kind() else {
                    return Err(WireError::ValueContractMismatch);
                };
                if terminal.len()
                    > usize::try_from(self.binding.limits().maximum_payload_bytes)
                        .map_err(|_| WireError::InvalidLimits)?
                {
                    return Err(WireError::OversizedPayload);
                }
                if conduit_core::primitive_info_kind(abnormal_kind.as_str()).is_none() {
                    return Err(WireError::ValueContractMismatch);
                }
                conduit_core::validate_primitive_info(abnormal_kind.as_str(), terminal)
                    .map_err(|_| WireError::ValueContractMismatch)?;
                let terminal_digest =
                    conduit_core::semantic_digest(abnormal_kind.as_str(), terminal);
                if self.source_direction(direction) && self.replay_input_terminal {
                    if self.input_closed
                        && self.input_abnormal
                        && final_sequence == self.next_sequence
                        && self.abnormal_terminal_digest == Some(terminal_digest)
                    {
                        self.replay_input_terminal = false;
                        return Ok(());
                    }
                    return Err(WireError::ValueContractMismatch);
                }
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
                self.input_abnormal = true;
                self.abnormal_terminal_digest = Some(terminal_digest);
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
    pub(in crate::session) fn check_hello(
        &self,
        direction: FrameDirection,
    ) -> Result<(), WireError> {
        if terminal_for(self, direction).is_some() {
            return Err(WireError::LateFrame);
        }
        if self.local_hello || self.peer_hello {
            let already_seen = match direction {
                FrameDirection::Outbound => self.local_hello,
                FrameDirection::Inbound => self.peer_hello,
            };
            if already_seen {
                return Err(WireError::DuplicateFrame);
            }
        }
        Ok(())
    }
    pub(in crate::session) fn mark_hello(
        &mut self,
        direction: FrameDirection,
    ) -> Result<(), WireError> {
        self.check_hello(direction)?;
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
pub(in crate::session) enum FrameDirection {
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

fn terminal_for<B>(
    machine: &SessionMachine<B>,
    direction: FrameDirection,
) -> Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => machine.local_terminal,
        FrameDirection::Inbound => machine.peer_terminal,
    }
}

fn terminal_for_mut<B>(
    machine: &mut SessionMachine<B>,
    direction: FrameDirection,
) -> &mut Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => &mut machine.local_terminal,
        FrameDirection::Inbound => &mut machine.peer_terminal,
    }
}
