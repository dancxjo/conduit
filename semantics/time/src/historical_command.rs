//! Versioned commands for an ordinary bounded historical timeline operation.

use alloc::string::String;
use conduit_core::{semantic_digest, BoundedResourceRef, TemporalInstant, TemporalScale};

use crate::{
    BoundedHistoricalTimeline, HistoricalEntryOrigin, HistoricalTimelineEntry,
    HistoricalTimelineRefusal, MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES,
};

pub const HISTORICAL_TIMELINE_COMMAND_INFO_ID: &str = "history/timeline-command@1";
pub const HISTORICAL_TIMELINE_COMMAND_VERSION: u8 = 1;
pub const MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES: usize = 1_024;
const MAGIC: [u8; 4] = *b"CHTC";
const DIGEST_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
// The largest variants are deliberately inline: their exact finite bound is
// preferable to hidden heap indirection at the future Play boundary.
#[allow(clippy::large_enum_variant)]
pub enum HistoricalTimelineCommand {
    Append {
        identity: String,
        event_time: TemporalInstant,
        origin: HistoricalEntryOrigin,
        value: BoundedResourceRef,
    },
    Remove {
        sequence: u64,
    },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum HistoricalTimelineOutcome {
    Appended { sequence: u64 },
    Removed(HistoricalTimelineEntry),
    Cleared { revision: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalTimelineCommandCodecRefusal {
    OutputTooSmall,
    CommandTooLarge,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    InvalidCommand,
    InvalidUtf8,
    InvalidIdentity,
    InvalidTime,
    InvalidResource,
    Integrity,
    TrailingBytes,
}

impl BoundedHistoricalTimeline {
    pub fn apply(
        &mut self,
        command: HistoricalTimelineCommand,
    ) -> Result<HistoricalTimelineOutcome, HistoricalTimelineRefusal> {
        match command {
            HistoricalTimelineCommand::Append {
                identity,
                event_time,
                origin,
                value,
            } => self
                .append(identity, event_time, origin, value)
                .map(|sequence| HistoricalTimelineOutcome::Appended { sequence }),
            HistoricalTimelineCommand::Remove { sequence } => self
                .remove(sequence)
                .map(HistoricalTimelineOutcome::Removed),
            HistoricalTimelineCommand::Clear => {
                self.clear()?;
                Ok(HistoricalTimelineOutcome::Cleared {
                    revision: self.clear_revision(),
                })
            }
        }
    }
}

pub fn encode_historical_timeline_command_into(
    command: &HistoricalTimelineCommand,
    output: &mut [u8],
) -> Result<usize, HistoricalTimelineCommandCodecRefusal> {
    let mut writer = Writer::new(output);
    writer.bytes(&MAGIC)?;
    writer.u8(HISTORICAL_TIMELINE_COMMAND_VERSION)?;
    match command {
        HistoricalTimelineCommand::Append {
            identity,
            event_time,
            origin,
            value,
        } => {
            if identity.is_empty() || identity.len() > MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES {
                return Err(HistoricalTimelineCommandCodecRefusal::InvalidIdentity);
            }
            event_time
                .validate()
                .map_err(|_| HistoricalTimelineCommandCodecRefusal::InvalidTime)?;
            let resource = value
                .encode()
                .map_err(|_| HistoricalTimelineCommandCodecRefusal::InvalidResource)?;
            writer.u8(0)?;
            writer.text(identity)?;
            writer.u64(event_time.ticks)?;
            writer.u8(encode_scale(event_time.scale))?;
            writer.text(&event_time.clock_basis)?;
            writer.u64(event_time.resolution_ticks)?;
            writer.u64(event_time.uncertainty_ticks)?;
            writer.u8(match origin {
                HistoricalEntryOrigin::MachineObservation => 0,
                HistoricalEntryOrigin::OperatorAuthored => 1,
            })?;
            writer.length_prefixed(&resource)?;
        }
        HistoricalTimelineCommand::Remove { sequence } => {
            writer.u8(1)?;
            writer.u64(*sequence)?;
        }
        HistoricalTimelineCommand::Clear => writer.u8(2)?,
    }
    writer.finish()
}

pub fn decode_historical_timeline_command(
    encoded: &[u8],
) -> Result<HistoricalTimelineCommand, HistoricalTimelineCommandCodecRefusal> {
    if encoded.len() > MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES {
        return Err(HistoricalTimelineCommandCodecRefusal::CommandTooLarge);
    }
    if encoded.len() < DIGEST_BYTES {
        return Err(HistoricalTimelineCommandCodecRefusal::Truncated);
    }
    let (payload, digest) = encoded.split_at(encoded.len() - DIGEST_BYTES);
    if semantic_digest(HISTORICAL_TIMELINE_COMMAND_INFO_ID, payload).as_slice() != digest {
        return Err(HistoricalTimelineCommandCodecRefusal::Integrity);
    }
    let mut cursor = Cursor::new(payload);
    if cursor.take(4)? != MAGIC {
        return Err(HistoricalTimelineCommandCodecRefusal::InvalidMagic);
    }
    if cursor.u8()? != HISTORICAL_TIMELINE_COMMAND_VERSION {
        return Err(HistoricalTimelineCommandCodecRefusal::UnsupportedVersion);
    }
    let command = match cursor.u8()? {
        0 => {
            let identity = cursor.text()?;
            if identity.is_empty() || identity.len() > MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES {
                return Err(HistoricalTimelineCommandCodecRefusal::InvalidIdentity);
            }
            let event_time = TemporalInstant {
                ticks: cursor.u64()?,
                scale: decode_scale(cursor.u8()?)?,
                clock_basis: cursor.text()?,
                resolution_ticks: cursor.u64()?,
                uncertainty_ticks: cursor.u64()?,
            };
            event_time
                .validate()
                .map_err(|_| HistoricalTimelineCommandCodecRefusal::InvalidTime)?;
            let origin = match cursor.u8()? {
                0 => HistoricalEntryOrigin::MachineObservation,
                1 => HistoricalEntryOrigin::OperatorAuthored,
                _ => return Err(HistoricalTimelineCommandCodecRefusal::InvalidCommand),
            };
            let value = BoundedResourceRef::decode(cursor.length_prefixed()?)
                .map_err(|_| HistoricalTimelineCommandCodecRefusal::InvalidResource)?;
            HistoricalTimelineCommand::Append {
                identity,
                event_time,
                origin,
                value,
            }
        }
        1 => HistoricalTimelineCommand::Remove {
            sequence: cursor.u64()?,
        },
        2 => HistoricalTimelineCommand::Clear,
        _ => return Err(HistoricalTimelineCommandCodecRefusal::InvalidCommand),
    };
    if !cursor.remaining().is_empty() {
        return Err(HistoricalTimelineCommandCodecRefusal::TrailingBytes);
    }
    Ok(command)
}

fn encode_scale(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn decode_scale(value: u8) -> Result<TemporalScale, HistoricalTimelineCommandCodecRefusal> {
    match value {
        0 => Ok(TemporalScale::Seconds),
        1 => Ok(TemporalScale::Milliseconds),
        2 => Ok(TemporalScale::Microseconds),
        3 => Ok(TemporalScale::Nanoseconds),
        _ => Err(HistoricalTimelineCommandCodecRefusal::InvalidTime),
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self {
            output,
            position: 0,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        let end = self
            .position
            .checked_add(value.len())
            .ok_or(HistoricalTimelineCommandCodecRefusal::CommandTooLarge)?;
        if end > MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES {
            return Err(HistoricalTimelineCommandCodecRefusal::CommandTooLarge);
        }
        if end > self.output.len() {
            return Err(HistoricalTimelineCommandCodecRefusal::OutputTooSmall);
        }
        self.output[self.position..end].copy_from_slice(value);
        self.position = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        self.bytes(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        let length = u16::try_from(value.len())
            .map_err(|_| HistoricalTimelineCommandCodecRefusal::CommandTooLarge)?;
        self.u16(length)?;
        self.bytes(value.as_bytes())
    }

    fn length_prefixed(
        &mut self,
        value: &[u8],
    ) -> Result<(), HistoricalTimelineCommandCodecRefusal> {
        let length = u16::try_from(value.len())
            .map_err(|_| HistoricalTimelineCommandCodecRefusal::CommandTooLarge)?;
        self.u16(length)?;
        self.bytes(value)
    }

    fn finish(mut self) -> Result<usize, HistoricalTimelineCommandCodecRefusal> {
        let digest = semantic_digest(
            HISTORICAL_TIMELINE_COMMAND_INFO_ID,
            &self.output[..self.position],
        );
        self.bytes(&digest)?;
        Ok(self.position)
    }
}

struct Cursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], HistoricalTimelineCommandCodecRefusal> {
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.encoded.len())
            .ok_or(HistoricalTimelineCommandCodecRefusal::Truncated)?;
        let value = &self.encoded[self.position..end];
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, HistoricalTimelineCommandCodecRefusal> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, HistoricalTimelineCommandCodecRefusal> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, HistoricalTimelineCommandCodecRefusal> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn text(&mut self) -> Result<String, HistoricalTimelineCommandCodecRefusal> {
        let length = usize::from(self.u16()?);
        let bytes = self.take(length)?;
        core::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|_| HistoricalTimelineCommandCodecRefusal::InvalidUtf8)
    }

    fn length_prefixed(&mut self) -> Result<&'a [u8], HistoricalTimelineCommandCodecRefusal> {
        let length = usize::from(self.u16()?);
        self.take(length)
    }

    fn remaining(&self) -> &'a [u8] {
        &self.encoded[self.position..]
    }
}
