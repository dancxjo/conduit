//! Versioned snapshot codec for complete bounded historical-timeline truth.

use alloc::{string::String, vec::Vec};
use conduit_core::{semantic_digest, BoundedResourceRef, KindId, TemporalInstant, TemporalScale};

use crate::{
    BoundedHistoricalTimeline, HistoricalEntryOrigin, HistoricalOverflowPolicy,
    HistoricalRetentionGap, HistoricalTimelineEntry, HistoricalTimelineRefusal,
    MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES, MAXIMUM_HISTORICAL_TIMELINE_ENTRIES,
};

pub const HISTORICAL_TIMELINE_SNAPSHOT_VERSION: u8 = 1;
/// Covers 64 maximally encoded entries, including their exact temporal and
/// resource-reference identities, plus complete timeline configuration.
pub const MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES: usize = 64 * 1024;
const MAGIC: [u8; 4] = *b"CHTL";
const DIGEST_BYTES: usize = 32;
const SNAPSHOT_DIGEST_DOMAIN: &str = "history/timeline-snapshot@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalTimelineCodecRefusal {
    Timeline(HistoricalTimelineRefusal),
    Resource,
    OutputTooSmall,
    SnapshotTooLarge,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    InvalidUtf8,
    InvalidEnum,
    Integrity,
    TrailingBytes,
}

pub fn encode_historical_timeline_into(
    timeline: &BoundedHistoricalTimeline,
    output: &mut [u8],
) -> Result<usize, HistoricalTimelineCodecRefusal> {
    let (profile, clock, scale, maximum_entries, maximum_bytes, overflow, next, clear) =
        timeline.snapshot_configuration();
    let mut writer = Writer::new(output);
    writer.bytes(&MAGIC)?;
    writer.u8(HISTORICAL_TIMELINE_SNAPSHOT_VERSION)?;
    writer.text(profile.as_str())?;
    writer.text(clock)?;
    writer.u8(encode_scale(scale))?;
    writer.u16(maximum_entries as u16)?;
    writer.u64(maximum_bytes)?;
    writer.u8(match overflow {
        HistoricalOverflowPolicy::Refuse => 0,
        HistoricalOverflowPolicy::EvictOldestWithGap => 1,
    })?;
    writer.u64(next)?;
    writer.u64(clear)?;
    match timeline.retention_gap() {
        None => writer.u8(0)?,
        Some(gap) => {
            writer.u8(1)?;
            for value in [
                gap.first_sequence,
                gap.last_sequence,
                gap.entries,
                gap.referenced_bytes,
            ] {
                writer.u64(value)?;
            }
        }
    }
    writer.u16(timeline.len() as u16)?;
    for index in 0..timeline.len() {
        let entry = timeline.entry(index).expect("retained entry");
        writer.u64(entry.sequence)?;
        writer.text(&entry.identity)?;
        writer.u64(entry.event_time.ticks)?;
        writer.u8(encode_scale(entry.event_time.scale))?;
        writer.text(&entry.event_time.clock_basis)?;
        writer.u64(entry.event_time.resolution_ticks)?;
        writer.u64(entry.event_time.uncertainty_ticks)?;
        writer.u8(match entry.origin {
            HistoricalEntryOrigin::MachineObservation => 0,
            HistoricalEntryOrigin::OperatorAuthored => 1,
        })?;
        let resource = entry
            .value
            .encode()
            .map_err(|_| HistoricalTimelineCodecRefusal::Resource)?;
        writer.u16(resource.len() as u16)?;
        writer.bytes(&resource)?;
    }
    writer.finish_digest()
}

pub fn decode_historical_timeline(
    encoded: &[u8],
) -> Result<BoundedHistoricalTimeline, HistoricalTimelineCodecRefusal> {
    if encoded.len() > MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES {
        return Err(HistoricalTimelineCodecRefusal::SnapshotTooLarge);
    }
    if encoded.len() < DIGEST_BYTES {
        return Err(HistoricalTimelineCodecRefusal::Truncated);
    }
    let (payload, encoded_digest) = encoded.split_at(encoded.len() - DIGEST_BYTES);
    if semantic_digest(SNAPSHOT_DIGEST_DOMAIN, payload).as_slice() != encoded_digest {
        return Err(HistoricalTimelineCodecRefusal::Integrity);
    }
    let mut cursor = Cursor::new(payload);
    if cursor.take(4)? != MAGIC {
        return Err(HistoricalTimelineCodecRefusal::InvalidMagic);
    }
    if cursor.u8()? != HISTORICAL_TIMELINE_SNAPSHOT_VERSION {
        return Err(HistoricalTimelineCodecRefusal::UnsupportedVersion);
    }
    let profile = KindId::from(cursor.text()?);
    let clock = cursor.text()?;
    let scale = decode_scale(cursor.u8()?)?;
    let maximum_entries = usize::from(cursor.u16()?);
    let maximum_bytes = cursor.u64()?;
    let overflow = match cursor.u8()? {
        0 => HistoricalOverflowPolicy::Refuse,
        1 => HistoricalOverflowPolicy::EvictOldestWithGap,
        _ => return Err(HistoricalTimelineCodecRefusal::InvalidEnum),
    };
    let next = cursor.u64()?;
    let clear = cursor.u64()?;
    let gap = match cursor.u8()? {
        0 => None,
        1 => Some(HistoricalRetentionGap {
            first_sequence: cursor.u64()?,
            last_sequence: cursor.u64()?,
            entries: cursor.u64()?,
            referenced_bytes: cursor.u64()?,
        }),
        _ => return Err(HistoricalTimelineCodecRefusal::InvalidEnum),
    };
    let count = usize::from(cursor.u16()?);
    if count > MAXIMUM_HISTORICAL_TIMELINE_ENTRIES {
        return Err(HistoricalTimelineCodecRefusal::Timeline(
            HistoricalTimelineRefusal::InvalidSnapshot,
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let sequence = cursor.u64()?;
        let identity = cursor.text()?;
        if identity.len() > MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES {
            return Err(HistoricalTimelineCodecRefusal::Timeline(
                HistoricalTimelineRefusal::InvalidEntryIdentity,
            ));
        }
        let event_time = TemporalInstant {
            ticks: cursor.u64()?,
            scale: decode_scale(cursor.u8()?)?,
            clock_basis: cursor.text()?,
            resolution_ticks: cursor.u64()?,
            uncertainty_ticks: cursor.u64()?,
        };
        let origin = match cursor.u8()? {
            0 => HistoricalEntryOrigin::MachineObservation,
            1 => HistoricalEntryOrigin::OperatorAuthored,
            _ => return Err(HistoricalTimelineCodecRefusal::InvalidEnum),
        };
        let resource = BoundedResourceRef::decode(cursor.length_prefixed()?)
            .map_err(|_| HistoricalTimelineCodecRefusal::Resource)?;
        entries.push(HistoricalTimelineEntry {
            sequence,
            identity,
            event_time,
            origin,
            value: resource,
        });
    }
    if !cursor.remaining().is_empty() {
        return Err(HistoricalTimelineCodecRefusal::TrailingBytes);
    }
    BoundedHistoricalTimeline::restore(
        profile,
        &clock,
        scale,
        maximum_entries,
        maximum_bytes,
        overflow,
        next,
        clear,
        gap,
        entries,
    )
    .map_err(HistoricalTimelineCodecRefusal::Timeline)
}

struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, cursor: 0 }
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), HistoricalTimelineCodecRefusal> {
        let end = self
            .cursor
            .checked_add(value.len())
            .ok_or(HistoricalTimelineCodecRefusal::SnapshotTooLarge)?;
        if end > MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES {
            return Err(HistoricalTimelineCodecRefusal::SnapshotTooLarge);
        }
        if end > self.output.len() {
            return Err(HistoricalTimelineCodecRefusal::OutputTooSmall);
        }
        self.output[self.cursor..end].copy_from_slice(value);
        self.cursor = end;
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<(), HistoricalTimelineCodecRefusal> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), HistoricalTimelineCodecRefusal> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), HistoricalTimelineCodecRefusal> {
        self.bytes(&value.to_le_bytes())
    }
    fn text(&mut self, value: &str) -> Result<(), HistoricalTimelineCodecRefusal> {
        let length = u16::try_from(value.len())
            .map_err(|_| HistoricalTimelineCodecRefusal::SnapshotTooLarge)?;
        self.u16(length)?;
        self.bytes(value.as_bytes())
    }
    fn finish_digest(mut self) -> Result<usize, HistoricalTimelineCodecRefusal> {
        let digest = semantic_digest(SNAPSHOT_DIGEST_DOMAIN, &self.output[..self.cursor]);
        self.bytes(&digest)?;
        Ok(self.cursor)
    }
}

fn encode_scale(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn decode_scale(value: u8) -> Result<TemporalScale, HistoricalTimelineCodecRefusal> {
    match value {
        0 => Ok(TemporalScale::Seconds),
        1 => Ok(TemporalScale::Milliseconds),
        2 => Ok(TemporalScale::Microseconds),
        3 => Ok(TemporalScale::Nanoseconds),
        _ => Err(HistoricalTimelineCodecRefusal::InvalidEnum),
    }
}

struct Cursor<'a> {
    remaining: &'a [u8],
}
impl<'a> Cursor<'a> {
    fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], HistoricalTimelineCodecRefusal> {
        if self.remaining.len() < length {
            return Err(HistoricalTimelineCodecRefusal::Truncated);
        }
        let (value, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, HistoricalTimelineCodecRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, HistoricalTimelineCodecRefusal> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, HistoricalTimelineCodecRefusal> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<String, HistoricalTimelineCodecRefusal> {
        let length = usize::from(self.u16()?);
        let bytes = self.take(length)?;
        core::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|_| HistoricalTimelineCodecRefusal::InvalidUtf8)
    }
    fn length_prefixed(&mut self) -> Result<&'a [u8], HistoricalTimelineCodecRefusal> {
        let length = usize::from(self.u16()?);
        self.take(length)
    }
    fn remaining(&self) -> &'a [u8] {
        self.remaining
    }
}
