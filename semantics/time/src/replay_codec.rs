//! Bounded canonical transport for retained replay timeline metadata.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    BoundedResourceRef, TemporalInstant, TemporalScale, MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

use crate::{
    HistoricalEntryOrigin, HistoricalReplayEntry, MAXIMUM_REPLAY_ENTRIES,
    MAXIMUM_REPLAY_IDENTITY_BYTES,
};

pub const REPLAY_TIMELINE_WIRE_VERSION: u8 = 1;
pub const MAXIMUM_REPLAY_TIMELINE_BYTES: usize = 7 + MAXIMUM_REPLAY_ENTRIES
    * (40
        + MAXIMUM_REPLAY_IDENTITY_BYTES
        + MAXIMUM_TEMPORAL_IDENTITY_BYTES
        + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES);
const MAGIC: [u8; 4] = *b"CRTL";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayTimelineCodecRefusal {
    EmptyTimeline,
    TooManyEntries,
    EmptyIdentity,
    IdentityTooLong,
    DuplicateIdentity,
    ReorderedHistoricalSequence,
    ReorderedHistoricalTime,
    InvalidHistoricalTime,
    IncomparableHistoricalTime,
    InvalidResource,
    InconsistentValueProfile,
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    InvalidUtf8,
    UnknownTimeScale,
    UnknownOrigin,
    TrailingBytes,
}

pub(crate) struct ReplayEntryFields<'a> {
    pub sequence: u64,
    pub identity: &'a str,
    pub event_time: &'a TemporalInstant,
    pub origin: HistoricalEntryOrigin,
    pub value: &'a BoundedResourceRef,
}

pub fn encode_replay_timeline_into(
    entries: &[HistoricalReplayEntry],
    output: &mut [u8],
) -> Result<usize, ReplayTimelineCodecRefusal> {
    encode_replay_timeline_fields_into(
        entries.len(),
        |index| ReplayEntryFields {
            sequence: entries[index].sequence,
            identity: &entries[index].identity,
            event_time: &entries[index].event_time,
            origin: entries[index].origin,
            value: &entries[index].value,
        },
        output,
    )
}

pub(crate) fn encode_replay_timeline_fields_into<'a>(
    count: usize,
    entry_at: impl Fn(usize) -> ReplayEntryFields<'a>,
    output: &mut [u8],
) -> Result<usize, ReplayTimelineCodecRefusal> {
    validate_entry_fields(count, &entry_at)?;
    let required = 7
        + (0..count)
            .map(|index| {
                let entry = entry_at(index);
                40 + entry.identity.len()
                    + entry.event_time.clock_basis.len()
                    + entry
                        .value
                        .encode()
                        .expect("validated replay resource remains encodable")
                        .len()
            })
            .sum::<usize>();
    if output.len() < required {
        return Err(ReplayTimelineCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = REPLAY_TIMELINE_WIRE_VERSION;
    output[5..7].copy_from_slice(&(count as u16).to_le_bytes());
    let mut cursor = 7;
    for index in 0..count {
        let entry = entry_at(index);
        output[cursor..cursor + 8].copy_from_slice(&entry.sequence.to_le_bytes());
        cursor += 8;
        let identity = entry.identity.as_bytes();
        output[cursor..cursor + 2].copy_from_slice(&(identity.len() as u16).to_le_bytes());
        cursor += 2;
        output[cursor..cursor + identity.len()].copy_from_slice(identity);
        cursor += identity.len();
        output[cursor..cursor + 8].copy_from_slice(&entry.event_time.ticks.to_le_bytes());
        cursor += 8;
        output[cursor] = encode_scale(entry.event_time.scale);
        cursor += 1;
        let basis = entry.event_time.clock_basis.as_bytes();
        output[cursor..cursor + 2].copy_from_slice(&(basis.len() as u16).to_le_bytes());
        cursor += 2;
        output[cursor..cursor + basis.len()].copy_from_slice(basis);
        cursor += basis.len();
        output[cursor..cursor + 8]
            .copy_from_slice(&entry.event_time.resolution_ticks.to_le_bytes());
        cursor += 8;
        output[cursor..cursor + 8]
            .copy_from_slice(&entry.event_time.uncertainty_ticks.to_le_bytes());
        cursor += 8;
        output[cursor] = encode_origin(entry.origin);
        cursor += 1;
        let resource = entry
            .value
            .encode()
            .expect("validated replay resource remains encodable");
        output[cursor..cursor + 2].copy_from_slice(&(resource.len() as u16).to_le_bytes());
        cursor += 2;
        output[cursor..cursor + resource.len()].copy_from_slice(&resource);
        cursor += resource.len();
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
        let sequence = read_u64(encoded, &mut cursor)?;
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
        let ticks = read_u64(encoded, &mut cursor)?;
        let scale = decode_scale(read_u8(encoded, &mut cursor)?)?;
        let basis_length = usize::from(read_u16(encoded, &mut cursor)?);
        if basis_length == 0 || basis_length > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
            return Err(ReplayTimelineCodecRefusal::InvalidHistoricalTime);
        }
        let basis_end = cursor
            .checked_add(basis_length)
            .filter(|end| *end <= encoded.len())
            .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
        let clock_basis = core::str::from_utf8(&encoded[cursor..basis_end])
            .map_err(|_| ReplayTimelineCodecRefusal::InvalidUtf8)?;
        cursor = basis_end;
        let resolution_ticks = read_u64(encoded, &mut cursor)?;
        let uncertainty_ticks = read_u64(encoded, &mut cursor)?;
        let origin = decode_origin(read_u8(encoded, &mut cursor)?)?;
        let resource_length = usize::from(read_u16(encoded, &mut cursor)?);
        if resource_length > MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES {
            return Err(ReplayTimelineCodecRefusal::InvalidResource);
        }
        let resource_end = cursor
            .checked_add(resource_length)
            .filter(|end| *end <= encoded.len())
            .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
        let value = BoundedResourceRef::decode(&encoded[cursor..resource_end])
            .map_err(|_| ReplayTimelineCodecRefusal::InvalidResource)?;
        cursor = resource_end;
        entries.push(HistoricalReplayEntry {
            sequence,
            identity: String::from(identity),
            event_time: TemporalInstant {
                ticks,
                scale,
                clock_basis: String::from(clock_basis),
                resolution_ticks,
                uncertainty_ticks,
            },
            origin,
            value,
        });
    }
    if cursor != encoded.len() {
        return Err(ReplayTimelineCodecRefusal::TrailingBytes);
    }
    validate_entries(&entries)?;
    Ok(entries)
}

fn validate_entries(entries: &[HistoricalReplayEntry]) -> Result<(), ReplayTimelineCodecRefusal> {
    validate_entry_fields(entries.len(), &|index| ReplayEntryFields {
        sequence: entries[index].sequence,
        identity: &entries[index].identity,
        event_time: &entries[index].event_time,
        origin: entries[index].origin,
        value: &entries[index].value,
    })
}

fn validate_entry_fields<'a>(
    count: usize,
    entry_at: &impl Fn(usize) -> ReplayEntryFields<'a>,
) -> Result<(), ReplayTimelineCodecRefusal> {
    if count == 0 {
        return Err(ReplayTimelineCodecRefusal::EmptyTimeline);
    }
    if count > MAXIMUM_REPLAY_ENTRIES {
        return Err(ReplayTimelineCodecRefusal::TooManyEntries);
    }
    for index in 0..count {
        let entry = entry_at(index);
        if entry.identity.is_empty() {
            return Err(ReplayTimelineCodecRefusal::EmptyIdentity);
        }
        if entry.identity.len() > MAXIMUM_REPLAY_IDENTITY_BYTES {
            return Err(ReplayTimelineCodecRefusal::IdentityTooLong);
        }
        entry
            .event_time
            .validate()
            .map_err(|_| ReplayTimelineCodecRefusal::InvalidHistoricalTime)?;
        let first = entry_at(0);
        if index > 0
            && (entry.event_time.clock_basis != first.event_time.clock_basis
                || entry.event_time.scale != first.event_time.scale)
        {
            return Err(ReplayTimelineCodecRefusal::IncomparableHistoricalTime);
        }
        if index > 0 && entry.event_time.ticks < entry_at(index - 1).event_time.ticks {
            return Err(ReplayTimelineCodecRefusal::ReorderedHistoricalTime);
        }
        if (0..index).any(|prior| entry_at(prior).identity == entry.identity) {
            return Err(ReplayTimelineCodecRefusal::DuplicateIdentity);
        }
        if index > 0 && entry.sequence <= entry_at(index - 1).sequence {
            return Err(ReplayTimelineCodecRefusal::ReorderedHistoricalSequence);
        }
        entry
            .value
            .encode()
            .map_err(|_| ReplayTimelineCodecRefusal::InvalidResource)?;
        if index > 0 && entry.value.content_profile != first.value.content_profile {
            return Err(ReplayTimelineCodecRefusal::InconsistentValueProfile);
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

fn read_u8(encoded: &[u8], cursor: &mut usize) -> Result<u8, ReplayTimelineCodecRefusal> {
    let value = *encoded
        .get(*cursor)
        .ok_or(ReplayTimelineCodecRefusal::Truncated)?;
    *cursor += 1;
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

fn encode_scale(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn decode_scale(value: u8) -> Result<TemporalScale, ReplayTimelineCodecRefusal> {
    match value {
        0 => Ok(TemporalScale::Seconds),
        1 => Ok(TemporalScale::Milliseconds),
        2 => Ok(TemporalScale::Microseconds),
        3 => Ok(TemporalScale::Nanoseconds),
        _ => Err(ReplayTimelineCodecRefusal::UnknownTimeScale),
    }
}

fn encode_origin(origin: HistoricalEntryOrigin) -> u8 {
    match origin {
        HistoricalEntryOrigin::MachineObservation => 0,
        HistoricalEntryOrigin::OperatorAuthored => 1,
    }
}

fn decode_origin(value: u8) -> Result<HistoricalEntryOrigin, ReplayTimelineCodecRefusal> {
    match value {
        0 => Ok(HistoricalEntryOrigin::MachineObservation),
        1 => Ok(HistoricalEntryOrigin::OperatorAuthored),
        _ => Err(ReplayTimelineCodecRefusal::UnknownOrigin),
    }
}
