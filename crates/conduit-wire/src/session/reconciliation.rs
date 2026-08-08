use super::{SessionIdentity, SessionRole, WireError};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionTransferCheckpoint {
    None,
    Offered(u64),
    Accepted(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionCheckpoint {
    pub next_sequence: u64,
    pub transfer: SessionTransferCheckpoint,
    pub input_closed: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionCheckpointOffer<'a> {
    pub identity: SessionIdentity<'a>,
    pub checkpoint: SessionCheckpoint,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionResumeAction {
    Continue,
    ReplayOffered(u64),
    AwaitReplay(u64),
    AdvanceDelivered(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionCheckpointAcceptance {
    pub local: SessionCheckpoint,
    pub peer: SessionCheckpoint,
    pub action: SessionResumeAction,
    pub same_plan_continues: bool,
}

pub(super) fn reconcile_checkpoints(
    role: SessionRole,
    local: SessionCheckpoint,
    peer: SessionCheckpoint,
) -> Result<SessionResumeAction, WireError> {
    if local.input_closed != peer.input_closed {
        return Err(WireError::InvalidState);
    }
    if local == peer {
        return Ok(SessionResumeAction::Continue);
    }
    if local.input_closed {
        return Err(WireError::InvalidState);
    }
    match (role, local, peer) {
        (
            SessionRole::Source,
            SessionCheckpoint {
                next_sequence,
                transfer: SessionTransferCheckpoint::Offered(sequence),
                input_closed: false,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
            },
        ) if sequence == next_sequence && peer_next == next_sequence => {
            Ok(SessionResumeAction::ReplayOffered(sequence))
        }
        (
            SessionRole::Sink,
            SessionCheckpoint {
                next_sequence,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::Offered(sequence),
                input_closed: false,
            },
        ) if sequence == next_sequence && peer_next == next_sequence => {
            Ok(SessionResumeAction::AwaitReplay(sequence))
        }
        (
            SessionRole::Source,
            SessionCheckpoint {
                next_sequence,
                transfer: SessionTransferCheckpoint::Accepted(sequence),
                input_closed: false,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
            },
        ) if sequence == next_sequence && sequence.checked_add(1) == Some(peer_next) => {
            Ok(SessionResumeAction::AdvanceDelivered(sequence))
        }
        (
            SessionRole::Sink,
            SessionCheckpoint {
                next_sequence,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::Accepted(sequence),
                input_closed: false,
            },
        ) if sequence.checked_add(1) == Some(next_sequence) && peer_next == sequence => {
            Ok(SessionResumeAction::AdvanceDelivered(sequence))
        }
        _ => Err(WireError::InvalidState),
    }
}
