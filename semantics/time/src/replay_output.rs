//! Bounded canonical values emitted by the reusable replay controller.

use alloc::string::String;

use crate::{ReplayEmission, ReplayState, MAXIMUM_REPLAY_ENTRIES, MAXIMUM_REPLAY_IDENTITY_BYTES};

pub const MAXIMUM_REPLAY_EVENT_BYTES: usize = 25 + MAXIMUM_REPLAY_IDENTITY_BYTES;
pub const MAXIMUM_REPLAY_STATE_BYTES: usize = 8;

const EVENT_MAGIC: [u8; 4] = *b"REVT";
const STATE_MAGIC: [u8; 4] = *b"RSTA";
const VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedReplayEvent {
    pub ordinal: usize,
    pub historical_identity: String,
    pub historical_event_ticks: u64,
    pub playback_ticks: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayOutputCodecRefusal {
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    OrdinalOutOfBounds,
    EmptyIdentity,
    IdentityTooLong,
    InvalidUtf8,
    UnknownState,
    TrailingBytes,
}

pub fn encode_replay_event_into(
    event: ReplayEmission<'_>,
    output: &mut [u8],
) -> Result<usize, ReplayOutputCodecRefusal> {
    validate_event(event.ordinal, event.historical_identity)?;
    let required = 25 + event.historical_identity.len();
    if output.len() < required {
        return Err(ReplayOutputCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&EVENT_MAGIC);
    output[4] = VERSION;
    output[5..7].copy_from_slice(&(event.ordinal as u16).to_le_bytes());
    output[7..9].copy_from_slice(&(event.historical_identity.len() as u16).to_le_bytes());
    let identity_end = 9 + event.historical_identity.len();
    output[9..identity_end].copy_from_slice(event.historical_identity.as_bytes());
    output[identity_end..identity_end + 8]
        .copy_from_slice(&event.historical_event_ticks.to_le_bytes());
    output[identity_end + 8..identity_end + 16]
        .copy_from_slice(&event.playback_ticks.to_le_bytes());
    Ok(required)
}

pub fn decode_replay_event(encoded: &[u8]) -> Result<OwnedReplayEvent, ReplayOutputCodecRefusal> {
    validate_header(encoded, EVENT_MAGIC, 9)?;
    let ordinal = usize::from(u16::from_le_bytes([encoded[5], encoded[6]]));
    let identity_length = usize::from(u16::from_le_bytes([encoded[7], encoded[8]]));
    validate_event_fields(ordinal, identity_length)?;
    let required = 25usize
        .checked_add(identity_length)
        .ok_or(ReplayOutputCodecRefusal::IdentityTooLong)?;
    if encoded.len() < required {
        return Err(ReplayOutputCodecRefusal::Truncated);
    }
    if encoded.len() != required {
        return Err(ReplayOutputCodecRefusal::TrailingBytes);
    }
    let identity_end = 9 + identity_length;
    let historical_identity = core::str::from_utf8(&encoded[9..identity_end])
        .map_err(|_| ReplayOutputCodecRefusal::InvalidUtf8)?;
    Ok(OwnedReplayEvent {
        ordinal,
        historical_identity: String::from(historical_identity),
        historical_event_ticks: read_u64(encoded, identity_end),
        playback_ticks: read_u64(encoded, identity_end + 8),
    })
}

pub fn encode_replay_state_into(
    state: ReplayState,
    output: &mut [u8],
) -> Result<usize, ReplayOutputCodecRefusal> {
    let required = if matches!(state, ReplayState::Failed { .. }) {
        8
    } else {
        6
    };
    if output.len() < required {
        return Err(ReplayOutputCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&STATE_MAGIC);
    output[4] = VERSION;
    output[5] = match state {
        ReplayState::Stopped => 0,
        ReplayState::Running => 1,
        ReplayState::Paused => 2,
        ReplayState::Completed => 3,
        ReplayState::Failed { .. } => 4,
    };
    if let ReplayState::Failed { code } = state {
        output[6..8].copy_from_slice(&code.to_le_bytes());
    }
    Ok(required)
}

pub fn decode_replay_state(encoded: &[u8]) -> Result<ReplayState, ReplayOutputCodecRefusal> {
    validate_header(encoded, STATE_MAGIC, 6)?;
    let (state, required) = match encoded[5] {
        0 => (ReplayState::Stopped, 6),
        1 => (ReplayState::Running, 6),
        2 => (ReplayState::Paused, 6),
        3 => (ReplayState::Completed, 6),
        4 => {
            if encoded.len() < 8 {
                return Err(ReplayOutputCodecRefusal::Truncated);
            }
            (
                ReplayState::Failed {
                    code: u16::from_le_bytes([encoded[6], encoded[7]]),
                },
                8,
            )
        }
        _ => return Err(ReplayOutputCodecRefusal::UnknownState),
    };
    if encoded.len() != required {
        return Err(ReplayOutputCodecRefusal::TrailingBytes);
    }
    Ok(state)
}

fn validate_event(ordinal: usize, identity: &str) -> Result<(), ReplayOutputCodecRefusal> {
    validate_event_fields(ordinal, identity.len())
}

fn validate_event_fields(
    ordinal: usize,
    identity_length: usize,
) -> Result<(), ReplayOutputCodecRefusal> {
    if ordinal >= MAXIMUM_REPLAY_ENTRIES {
        return Err(ReplayOutputCodecRefusal::OrdinalOutOfBounds);
    }
    if identity_length == 0 {
        return Err(ReplayOutputCodecRefusal::EmptyIdentity);
    }
    if identity_length > MAXIMUM_REPLAY_IDENTITY_BYTES {
        return Err(ReplayOutputCodecRefusal::IdentityTooLong);
    }
    Ok(())
}

fn validate_header(
    encoded: &[u8],
    magic: [u8; 4],
    minimum: usize,
) -> Result<(), ReplayOutputCodecRefusal> {
    if encoded.len() < minimum {
        return Err(ReplayOutputCodecRefusal::Truncated);
    }
    if encoded[..4] != magic {
        return Err(ReplayOutputCodecRefusal::InvalidMagic);
    }
    if encoded[4] != VERSION {
        return Err(ReplayOutputCodecRefusal::UnsupportedVersion);
    }
    Ok(())
}

fn read_u64(encoded: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(
        encoded[start..start + 8]
            .try_into()
            .expect("the checked replay output slice is exact"),
    )
}
