//! Bounded canonical values emitted by the reusable replay controller.

use alloc::string::String;
use conduit_core::{
    BoundedResourceRef, TemporalInstant, TemporalScale, MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

use crate::{
    HistoricalEntryOrigin, ReplayEmission, ReplayState, MAXIMUM_REPLAY_ENTRIES,
    MAXIMUM_REPLAY_IDENTITY_BYTES,
};

pub const MAXIMUM_REPLAY_EVENT_BYTES: usize = 55
    + MAXIMUM_REPLAY_IDENTITY_BYTES
    + MAXIMUM_TEMPORAL_IDENTITY_BYTES
    + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES;
pub const MAXIMUM_REPLAY_STATE_BYTES: usize = 8;

const EVENT_MAGIC: [u8; 4] = *b"REVT";
const STATE_MAGIC: [u8; 4] = *b"RSTA";
const VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedReplayEvent {
    pub ordinal: usize,
    pub historical_sequence: u64,
    pub historical_identity: String,
    pub historical_event_time: TemporalInstant,
    pub historical_origin: HistoricalEntryOrigin,
    pub value: BoundedResourceRef,
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
    InvalidHistoricalTime,
    InvalidResource,
    UnknownTimeScale,
    UnknownOrigin,
    UnknownState,
    TrailingBytes,
}

pub fn encode_replay_event_into(
    event: ReplayEmission<'_>,
    output: &mut [u8],
) -> Result<usize, ReplayOutputCodecRefusal> {
    validate_event(event.ordinal, event.historical_identity)?;
    event
        .historical_event_time
        .validate()
        .map_err(|_| ReplayOutputCodecRefusal::InvalidHistoricalTime)?;
    let resource = event
        .value
        .encode()
        .map_err(|_| ReplayOutputCodecRefusal::InvalidResource)?;
    let required = 55
        + event.historical_identity.len()
        + event.historical_event_time.clock_basis.len()
        + resource.len();
    if output.len() < required {
        return Err(ReplayOutputCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&EVENT_MAGIC);
    output[4] = VERSION;
    output[5..7].copy_from_slice(&(event.ordinal as u16).to_le_bytes());
    output[7..9].copy_from_slice(&(event.historical_identity.len() as u16).to_le_bytes());
    let identity_end = 9 + event.historical_identity.len();
    output[9..identity_end].copy_from_slice(event.historical_identity.as_bytes());
    let mut cursor = identity_end;
    write_u64(output, &mut cursor, event.historical_sequence);
    write_u64(output, &mut cursor, event.historical_event_time.ticks);
    output[cursor] = encode_scale(event.historical_event_time.scale);
    cursor += 1;
    let basis = event.historical_event_time.clock_basis.as_bytes();
    output[cursor..cursor + 2].copy_from_slice(&(basis.len() as u16).to_le_bytes());
    cursor += 2;
    output[cursor..cursor + basis.len()].copy_from_slice(basis);
    cursor += basis.len();
    write_u64(
        output,
        &mut cursor,
        event.historical_event_time.resolution_ticks,
    );
    write_u64(
        output,
        &mut cursor,
        event.historical_event_time.uncertainty_ticks,
    );
    output[cursor] = encode_origin(event.historical_origin);
    cursor += 1;
    output[cursor..cursor + 2].copy_from_slice(&(resource.len() as u16).to_le_bytes());
    cursor += 2;
    output[cursor..cursor + resource.len()].copy_from_slice(&resource);
    cursor += resource.len();
    write_u64(output, &mut cursor, event.playback_ticks);
    Ok(required)
}

pub fn decode_replay_event(encoded: &[u8]) -> Result<OwnedReplayEvent, ReplayOutputCodecRefusal> {
    validate_header(encoded, EVENT_MAGIC, 9)?;
    let ordinal = usize::from(u16::from_le_bytes([encoded[5], encoded[6]]));
    let identity_length = usize::from(u16::from_le_bytes([encoded[7], encoded[8]]));
    validate_event_fields(ordinal, identity_length)?;
    let minimum = 55usize
        .checked_add(identity_length)
        .ok_or(ReplayOutputCodecRefusal::IdentityTooLong)?;
    if encoded.len() < minimum {
        return Err(ReplayOutputCodecRefusal::Truncated);
    }
    let identity_end = 9 + identity_length;
    let historical_identity = core::str::from_utf8(&encoded[9..identity_end])
        .map_err(|_| ReplayOutputCodecRefusal::InvalidUtf8)?;
    let mut cursor = identity_end;
    let historical_sequence = read_u64_at(encoded, &mut cursor)?;
    let ticks = read_u64_at(encoded, &mut cursor)?;
    let scale = decode_scale(
        *encoded
            .get(cursor)
            .ok_or(ReplayOutputCodecRefusal::Truncated)?,
    )?;
    cursor += 1;
    let basis_length = usize::from(read_u16_at(encoded, &mut cursor)?);
    if basis_length == 0 || basis_length > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
        return Err(ReplayOutputCodecRefusal::InvalidHistoricalTime);
    }
    let basis_end = cursor
        .checked_add(basis_length)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayOutputCodecRefusal::Truncated)?;
    let clock_basis = core::str::from_utf8(&encoded[cursor..basis_end])
        .map_err(|_| ReplayOutputCodecRefusal::InvalidUtf8)?;
    cursor = basis_end;
    let resolution_ticks = read_u64_at(encoded, &mut cursor)?;
    let uncertainty_ticks = read_u64_at(encoded, &mut cursor)?;
    let historical_origin = decode_origin(read_u8_at(encoded, &mut cursor)?)?;
    let resource_length = usize::from(read_u16_at(encoded, &mut cursor)?);
    if resource_length > MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES {
        return Err(ReplayOutputCodecRefusal::InvalidResource);
    }
    let resource_end = cursor
        .checked_add(resource_length)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayOutputCodecRefusal::Truncated)?;
    let value = BoundedResourceRef::decode(&encoded[cursor..resource_end])
        .map_err(|_| ReplayOutputCodecRefusal::InvalidResource)?;
    cursor = resource_end;
    let playback_ticks = read_u64_at(encoded, &mut cursor)?;
    if cursor != encoded.len() {
        return Err(ReplayOutputCodecRefusal::TrailingBytes);
    }
    let historical_event_time = TemporalInstant {
        ticks,
        scale,
        clock_basis: String::from(clock_basis),
        resolution_ticks,
        uncertainty_ticks,
    };
    historical_event_time
        .validate()
        .map_err(|_| ReplayOutputCodecRefusal::InvalidHistoricalTime)?;
    Ok(OwnedReplayEvent {
        ordinal,
        historical_sequence,
        historical_identity: String::from(historical_identity),
        historical_event_time,
        historical_origin,
        value,
        playback_ticks,
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

fn write_u64(output: &mut [u8], cursor: &mut usize, value: u64) {
    output[*cursor..*cursor + 8].copy_from_slice(&value.to_le_bytes());
    *cursor += 8;
}

fn read_u16_at(encoded: &[u8], cursor: &mut usize) -> Result<u16, ReplayOutputCodecRefusal> {
    let end = cursor
        .checked_add(2)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayOutputCodecRefusal::Truncated)?;
    let value = u16::from_le_bytes(encoded[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_u8_at(encoded: &[u8], cursor: &mut usize) -> Result<u8, ReplayOutputCodecRefusal> {
    let value = *encoded
        .get(*cursor)
        .ok_or(ReplayOutputCodecRefusal::Truncated)?;
    *cursor += 1;
    Ok(value)
}

fn read_u64_at(encoded: &[u8], cursor: &mut usize) -> Result<u64, ReplayOutputCodecRefusal> {
    let end = cursor
        .checked_add(8)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayOutputCodecRefusal::Truncated)?;
    let value = u64::from_le_bytes(encoded[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn encode_scale(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn encode_origin(origin: HistoricalEntryOrigin) -> u8 {
    match origin {
        HistoricalEntryOrigin::MachineObservation => 0,
        HistoricalEntryOrigin::OperatorAuthored => 1,
    }
}

fn decode_origin(value: u8) -> Result<HistoricalEntryOrigin, ReplayOutputCodecRefusal> {
    match value {
        0 => Ok(HistoricalEntryOrigin::MachineObservation),
        1 => Ok(HistoricalEntryOrigin::OperatorAuthored),
        _ => Err(ReplayOutputCodecRefusal::UnknownOrigin),
    }
}

fn decode_scale(value: u8) -> Result<TemporalScale, ReplayOutputCodecRefusal> {
    match value {
        0 => Ok(TemporalScale::Seconds),
        1 => Ok(TemporalScale::Milliseconds),
        2 => Ok(TemporalScale::Microseconds),
        3 => Ok(TemporalScale::Nanoseconds),
        _ => Err(ReplayOutputCodecRefusal::UnknownTimeScale),
    }
}
