use crate::{parser::MAXIMUM_SYSEX_BYTES, MidiMessage, ParsedMidi};

/// Protocol-domain value returned by an admitted MIDI input operation.
///
/// The timestamp is already correlated to the Plan's monotonic-microsecond
/// profile. It is not a MIDI wire fact and it does not make this value portable
/// musical meaning; the explicit input adapter performs that conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiInputObservation {
    pub event_time_micros: u64,
    pub parsed: ParsedMidi,
}

pub const MIDI_INPUT_OBSERVATION_INFO_ID: &str = "midi/input-observation@1";
pub const MIDI_INPUT_OBSERVATION_ENCODED_LEN: usize = 13;

const NOTE_OFF: u8 = 0;
const NOTE_ON: u8 = 1;
const CONTROL_CHANGE: u8 = 2;
const PITCH_BEND: u8 = 3;
const UNSUPPORTED_CHANNEL: u8 = 4;
const UNSUPPORTED_SYSTEM: u8 = 5;
const UNSUPPORTED_REALTIME: u8 = 6;
const UNSUPPORTED_SYSEX: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiObservationCodecError {
    WrongLength,
    InvalidTag,
    NonCanonicalPayload,
}

impl MidiInputObservation {
    pub fn encode(
        self,
    ) -> Result<[u8; MIDI_INPUT_OBSERVATION_ENCODED_LEN], MidiObservationCodecError> {
        validate(self.parsed)?;
        let mut encoded = [0; MIDI_INPUT_OBSERVATION_ENCODED_LEN];
        encoded[..8].copy_from_slice(&self.event_time_micros.to_le_bytes());
        match self.parsed {
            ParsedMidi::Message(MidiMessage::NoteOff {
                channel,
                key,
                velocity,
            }) => payload(&mut encoded, NOTE_OFF, [channel, key, velocity, 0]),
            ParsedMidi::Message(MidiMessage::NoteOn {
                channel,
                key,
                velocity,
            }) => payload(&mut encoded, NOTE_ON, [channel, key, velocity, 0]),
            ParsedMidi::Message(MidiMessage::ControlChange {
                channel,
                controller,
                value,
            }) => payload(
                &mut encoded,
                CONTROL_CHANGE,
                [channel, controller, value, 0],
            ),
            ParsedMidi::Message(MidiMessage::PitchBend { channel, value }) => payload(
                &mut encoded,
                PITCH_BEND,
                [channel, (value & 0x7f) as u8, (value >> 7) as u8, 0],
            ),
            ParsedMidi::Message(MidiMessage::UnsupportedChannel {
                status,
                first,
                second,
            }) => payload(
                &mut encoded,
                UNSUPPORTED_CHANNEL,
                [
                    status,
                    first,
                    second.unwrap_or(0),
                    u8::from(second.is_some()),
                ],
            ),
            ParsedMidi::UnsupportedSystem { status } => {
                payload(&mut encoded, UNSUPPORTED_SYSTEM, [status, 0, 0, 0]);
            }
            ParsedMidi::UnsupportedRealtime { status } => {
                payload(&mut encoded, UNSUPPORTED_REALTIME, [status, 0, 0, 0]);
            }
            ParsedMidi::UnsupportedSysEx { bytes } => {
                let bytes = bytes.to_le_bytes();
                payload(&mut encoded, UNSUPPORTED_SYSEX, [bytes[0], bytes[1], 0, 0]);
            }
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, MidiObservationCodecError> {
        let encoded: &[u8; MIDI_INPUT_OBSERVATION_ENCODED_LEN] = encoded
            .try_into()
            .map_err(|_| MidiObservationCodecError::WrongLength)?;
        let event_time_micros = u64::from_le_bytes(
            encoded[..8]
                .try_into()
                .map_err(|_| MidiObservationCodecError::WrongLength)?,
        );
        let data = [encoded[9], encoded[10], encoded[11], encoded[12]];
        let parsed = match encoded[8] {
            NOTE_OFF if data[3] == 0 => ParsedMidi::Message(MidiMessage::NoteOff {
                channel: data[0],
                key: data[1],
                velocity: data[2],
            }),
            NOTE_ON if data[3] == 0 => ParsedMidi::Message(MidiMessage::NoteOn {
                channel: data[0],
                key: data[1],
                velocity: data[2],
            }),
            CONTROL_CHANGE if data[3] == 0 => ParsedMidi::Message(MidiMessage::ControlChange {
                channel: data[0],
                controller: data[1],
                value: data[2],
            }),
            PITCH_BEND if data[3] == 0 => ParsedMidi::Message(MidiMessage::PitchBend {
                channel: data[0],
                value: u16::from(data[1]) | (u16::from(data[2]) << 7),
            }),
            UNSUPPORTED_CHANNEL if data[3] <= 1 => {
                ParsedMidi::Message(MidiMessage::UnsupportedChannel {
                    status: data[0],
                    first: data[1],
                    second: (data[3] == 1).then_some(data[2]),
                })
            }
            UNSUPPORTED_SYSTEM if data[1..] == [0, 0, 0] => {
                ParsedMidi::UnsupportedSystem { status: data[0] }
            }
            UNSUPPORTED_REALTIME if data[1..] == [0, 0, 0] => {
                ParsedMidi::UnsupportedRealtime { status: data[0] }
            }
            UNSUPPORTED_SYSEX if data[2..] == [0, 0] => ParsedMidi::UnsupportedSysEx {
                bytes: u16::from_le_bytes([data[0], data[1]]),
            },
            0..=UNSUPPORTED_SYSEX => return Err(MidiObservationCodecError::NonCanonicalPayload),
            _ => return Err(MidiObservationCodecError::InvalidTag),
        };
        validate(parsed)?;
        Ok(Self {
            event_time_micros,
            parsed,
        })
    }
}

fn payload(encoded: &mut [u8; MIDI_INPUT_OBSERVATION_ENCODED_LEN], tag: u8, data: [u8; 4]) {
    encoded[8] = tag;
    encoded[9..].copy_from_slice(&data);
}

fn validate(parsed: ParsedMidi) -> Result<(), MidiObservationCodecError> {
    let valid = match parsed {
        ParsedMidi::Message(MidiMessage::NoteOff {
            channel,
            key,
            velocity,
        })
        | ParsedMidi::Message(MidiMessage::NoteOn {
            channel,
            key,
            velocity,
        }) => channel <= 15 && key <= 127 && velocity <= 127,
        ParsedMidi::Message(MidiMessage::ControlChange {
            channel,
            controller,
            value,
        }) => channel <= 15 && controller <= 127 && value <= 127,
        ParsedMidi::Message(MidiMessage::PitchBend { channel, value }) => {
            channel <= 15 && value <= 16_383
        }
        ParsedMidi::Message(MidiMessage::UnsupportedChannel {
            status,
            first,
            second,
        }) => {
            let family = status & 0xf0;
            let expected_second = !matches!(family, 0xc0 | 0xd0);
            (0x80..=0xef).contains(&status)
                && !matches!(family, 0x80 | 0x90 | 0xb0 | 0xe0)
                && first <= 127
                && second.is_some() == expected_second
                && second.is_none_or(|value| value <= 127)
        }
        ParsedMidi::UnsupportedSystem { status } => (0xf1..=0xf7).contains(&status),
        ParsedMidi::UnsupportedRealtime { status } => status >= 0xf8,
        ParsedMidi::UnsupportedSysEx { bytes } => (2..=MAXIMUM_SYSEX_BYTES).contains(&bytes),
    };
    if valid {
        Ok(())
    } else {
        Err(MidiObservationCodecError::NonCanonicalPayload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_protocol_observation_has_one_exact_encoding() {
        let parsed = [
            ParsedMidi::Message(MidiMessage::NoteOff {
                channel: 15,
                key: 127,
                velocity: 64,
            }),
            ParsedMidi::Message(MidiMessage::NoteOn {
                channel: 0,
                key: 60,
                velocity: 127,
            }),
            ParsedMidi::Message(MidiMessage::ControlChange {
                channel: 2,
                controller: 64,
                value: 127,
            }),
            ParsedMidi::Message(MidiMessage::PitchBend {
                channel: 3,
                value: 16_383,
            }),
            ParsedMidi::Message(MidiMessage::UnsupportedChannel {
                status: 0xa4,
                first: 9,
                second: Some(10),
            }),
            ParsedMidi::Message(MidiMessage::UnsupportedChannel {
                status: 0xc4,
                first: 9,
                second: None,
            }),
            ParsedMidi::UnsupportedSystem { status: 0xf2 },
            ParsedMidi::UnsupportedRealtime { status: 0xf8 },
            ParsedMidi::UnsupportedSysEx { bytes: 256 },
        ];
        for parsed in parsed {
            let observation = MidiInputObservation {
                event_time_micros: 42,
                parsed,
            };
            let encoded = observation.encode().unwrap();
            assert_eq!(encoded.len(), MIDI_INPUT_OBSERVATION_ENCODED_LEN);
            assert_eq!(MidiInputObservation::decode(&encoded), Ok(observation));
        }
    }

    #[test]
    fn malformed_or_noncanonical_values_refuse() {
        assert_eq!(
            MidiInputObservation::decode(&[0; MIDI_INPUT_OBSERVATION_ENCODED_LEN - 1]),
            Err(MidiObservationCodecError::WrongLength)
        );
        let mut invalid = [0; MIDI_INPUT_OBSERVATION_ENCODED_LEN];
        invalid[8] = 8;
        assert_eq!(
            MidiInputObservation::decode(&invalid),
            Err(MidiObservationCodecError::InvalidTag)
        );
        invalid[8] = NOTE_ON;
        invalid[9] = 16;
        assert_eq!(
            MidiInputObservation::decode(&invalid),
            Err(MidiObservationCodecError::NonCanonicalPayload)
        );
        let invalid = MidiInputObservation {
            event_time_micros: 0,
            parsed: ParsedMidi::UnsupportedSysEx { bytes: 257 },
        };
        assert_eq!(
            invalid.encode(),
            Err(MidiObservationCodecError::NonCanonicalPayload)
        );
    }
}
