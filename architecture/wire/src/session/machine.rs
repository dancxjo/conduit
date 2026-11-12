use super::*;
use crate::{SessionRole, SessionTerminalDisposition, SessionTransferCheckpoint};

mod lifecycle;
pub use lifecycle::SessionMachine;
use lifecycle::*;
pub(in crate::session) use lifecycle::{FrameDirection, Scope};

impl SessionMachine {
    pub fn new(binding: SessionBinding, role: SessionRole) -> Result<Self, WireError> {
        binding.validate()?;
        Ok(Self::with_scope(binding, role))
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
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
            SessionResumeAction::ReplayInputTerminal => {
                self.replay_input_terminal = true;
            }
            SessionResumeAction::AwaitInputTerminal => {
                self.replay_input_terminal = false;
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
        validate_identity(&self.binding, frame.identity)?;
        if let SessionMessage::Hello(hello) = frame.message {
            return self.admit_hello(direction, hello);
        }
        self.admit_message(direction, frame.message)
    }

    fn admit_hello(
        &mut self,
        direction: FrameDirection,
        hello: SessionHello<'_>,
    ) -> Result<(), WireError> {
        self.check_hello(direction)?;
        validate_hello(&self.binding, hello)?;
        self.mark_hello(direction)
    }
}
