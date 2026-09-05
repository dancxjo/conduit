//! Bounded canonical transport for retained replay timeline metadata.

use alloc::{string::String, vec::Vec};

use crate::{HistoricalReplayEntry, MAXIMUM_REPLAY_ENTRIES, MAXIMUM_REPLAY_IDENTITY_BYTES};

pub const REPLAY_TIMELINE_WIRE_VERSION: u8 = 1;
pub const MAXIMUM_REPLAY_TIMELINE_BYTES: usize =
    7 + MAXIMUM_REPLAY_ENTRIES * (2 + MAXIMUM_REPLAY_IDENTITY_BYTES + 8);
const MAGIC: [u8; 4] = *b"CRTL";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayTimelineCodecRefusal {
    EmptyTimeline,
    TooManyEntries,
    EmptyIdentity,
    IdentityTooLong,
    DuplicateIdentity,
    ReorderedHistoricalTime,
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    InvalidUtf8,
    TrailingBytes,
}

pub fn encode_replay_timeline_into(
    entries: &[HistoricalReplayEntry],
    output: &mut [u8],
) -> Result<usize, ReplayTimelineCodecRefusal> {
    validate_entries(entries)?;
    let required = 7 + entries
        .iter()
        .map(|entry| 10 + entry.identity.len())
        .sum::<usize>();
    if output.len() < required {
        return Err(ReplayTimelineCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = REPLAY_TIMELINE_WIRE_VERSION;
    output[5..7].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut cursor = 7;
    for entry in entries {
        let identity = entry.identity.as_bytes();
        output[cursor..cursor + 2].copy_from_slice(&(identity.len() as u16).to_le_bytes());
        cursor += 2;
        output[cursor..cursor + identity.len()].copy_from_slice(identity);
        cursor += identity.len();
        output[cursor..cursor + 8].copy_from_slice(&entry.event_ticks.to_le_bytes());
        cursor += 8;
    }
    Ok(cursor)
}

pub fn decode_replay_timeline(
    encoded: &[u8],
) -> Result<Vec<HistoricalReplayEntry>, ReplayTimelineCodecRefusal> {
    if encoded.len() < 7 {
        return Err(ReplayTimelineCodecRefusal::Truncated);
    }
    if encoded[..4] != MAGIC {
        return Err(ReplayTimelineCodecRefusal::InvalidMagic);
    }
    if encoded[4] != REPLAY_TIMELINE_WIRE_VERSION {
        return Err(ReplayTimelineCodecRefusal::UnsupportedVersion);
    }
    let count = usize::from(u16::from_le_bytes([encoded[5], encoded[6]]));
    if count == 0 {
        return Err(ReplayTimelineCodecRefusal::EmptyTimeline);
    }
    if count > MAXIMUM_REPLAY_ENTRIES {
        return Err(ReplayTimelineCodecRefusal::TooManyEntries);
    }
    let mut entries = Vec::with_capacity(count);
    let mut cursor = 7;
    for _ in 0..count {
        let identity_length = usize::from(read_u16(encoded, &mut cursor)?);
        if identity_length == 0 {
            return Err(ReplayTimelineCodecRefusal::EmptyIdentity);
        }
        if identity_length > MAXIMUM_REPLAY_IDENTITY_BYTES {
            return Err(ReplayTimelineCodecRefusal::IdentityTooLong);
        }
        let identity_end = cursor
            .checked_add(identity_length)
            .filter(|end| *end <= encoded.len())
            .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
        let identity = core::str::from_utf8(&encoded[cursor..identity_end])
            .map_err(|_| ReplayTimelineCodecRefusal::InvalidUtf8)?;
        cursor = identity_end;
        let event_ticks = read_u64(encoded, &mut cursor)?;
        entries.push(HistoricalReplayEntry {
            identity: String::from(identity),
            event_ticks,
        });
    }
    if cursor != encoded.len() {
        return Err(ReplayTimelineCodecRefusal::TrailingBytes);
    }
    validate_entries(&entries)?;
    Ok(entries)
}

fn validate_entries(entries: &[HistoricalReplayEntry]) -> Result<(), ReplayTimelineCodecRefusal> {
    if entries.is_empty() {
        return Err(ReplayTimelineCodecRefusal::EmptyTimeline);
    }
    if entries.len() > MAXIMUM_REPLAY_ENTRIES {
        return Err(ReplayTimelineCodecRefusal::TooManyEntries);
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.identity.is_empty() {
            return Err(ReplayTimelineCodecRefusal::EmptyIdentity);
        }
        if entry.identity.len() > MAXIMUM_REPLAY_IDENTITY_BYTES {
            return Err(ReplayTimelineCodecRefusal::IdentityTooLong);
        }
        if index > 0 && entry.event_ticks < entries[index - 1].event_ticks {
            return Err(ReplayTimelineCodecRefusal::ReorderedHistoricalTime);
        }
        if entries[..index]
            .iter()
            .any(|prior| prior.identity == entry.identity)
        {
            return Err(ReplayTimelineCodecRefusal::DuplicateIdentity);
        }
    }
    Ok(())
}

fn read_u16(encoded: &[u8], cursor: &mut usize) -> Result<u16, ReplayTimelineCodecRefusal> {
    let end = cursor
        .checked_add(2)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
    let value = u16::from_le_bytes([encoded[*cursor], encoded[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn read_u64(encoded: &[u8], cursor: &mut usize) -> Result<u64, ReplayTimelineCodecRefusal> {
    let end = cursor
        .checked_add(8)
        .filter(|end| *end <= encoded.len())
        .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
    let value = u64::from_le_bytes(
        encoded[*cursor..end]
            .try_into()
            .expect("the checked replay tick slice is exact"),
    );
    *cursor = end;
    Ok(value)
}
