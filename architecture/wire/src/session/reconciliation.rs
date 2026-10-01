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
    /// The observed input terminal was semantic abnormal truth rather than
    /// normal closure. Valid only when `input_closed` is also true.
    pub input_abnormal: bool,
    /// Exact semantic identity of the typed abnormal terminal value. This is
    /// absent for an open or normally closed input.
    pub abnormal_terminal_digest: Option<[u8; 32]>,
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
    ReplayInputTerminal,
    AwaitInputTerminal,
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
    if local.input_abnormal && !local.input_closed
        || peer.input_abnormal && !peer.input_closed
        || local.input_abnormal != local.abnormal_terminal_digest.is_some()
        || peer.input_abnormal != peer.abnormal_terminal_digest.is_some()
    {
        return Err(WireError::InvalidState);
    }
    if local.input_closed != peer.input_closed {
        return match role {
            SessionRole::Source if local.input_closed && !peer.input_closed => {
                Ok(SessionResumeAction::ReplayInputTerminal)
            }
            SessionRole::Sink if !local.input_closed && peer.input_closed => {
                Ok(SessionResumeAction::AwaitInputTerminal)
            }
            _ => Err(WireError::InvalidState),
        };
    }
    if local.abnormal_terminal_digest != peer.abnormal_terminal_digest {
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
                input_abnormal: false,
                abnormal_terminal_digest: None,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
                input_abnormal: false,
                abnormal_terminal_digest: None,
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
                input_abnormal: false,
                abnormal_terminal_digest: None,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::Offered(sequence),
                input_closed: false,
                input_abnormal: false,
                abnormal_terminal_digest: None,
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
                input_abnormal: false,
                abnormal_terminal_digest: None,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::None,
                input_closed: false,
                input_abnormal: false,
                abnormal_terminal_digest: None,
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
                input_abnormal: false,
                abnormal_terminal_digest: None,
            },
            SessionCheckpoint {
                next_sequence: peer_next,
                transfer: SessionTransferCheckpoint::Accepted(sequence),
                input_closed: false,
                input_abnormal: false,
                abnormal_terminal_digest: None,
            },
        ) if sequence.checked_add(1) == Some(next_sequence) && peer_next == sequence => {
            Ok(SessionResumeAction::AdvanceDelivered(sequence))
        }
        _ => Err(WireError::InvalidState),
    }
}
