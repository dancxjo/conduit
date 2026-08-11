//! Portable sound and musical information carried by exact typed Ports.
//!
//! These values contain musical/media meaning only. MIDI keys, device names,
//! PCM handles, OPL registers, and host callback facts belong to realizations.

use crate::semantic_digest;

pub const SOUND_TONE_INFO_ID: &str = "sound/tone-intent@1";
pub const MUSIC_NOTE_INFO_ID: &str = "music/note-event@1";
pub const MUSIC_CONTROL_INFO_ID: &str = "music/control-event@1";

pub const TONE_INTENT_ENCODED_LEN: usize = 41;
pub const NOTE_EVENT_ENCODED_LEN: usize = 43;
pub const CONTROL_EVENT_ENCODED_LEN: usize = 22;

pub const MINIMUM_PITCH_MILLIHERTZ: u64 = 8_000;
pub const MAXIMUM_PITCH_MILLIHERTZ: u64 = 40_000_000;
pub const MINIMUM_A4_MILLIHERTZ: u64 = 400_000;
pub const MAXIMUM_A4_MILLIHERTZ: u64 = 480_000;
pub const MAXIMUM_ABSOLUTE_DETUNE_MICROCENTS: i32 = 24_000_000;
pub const MAXIMUM_PITCH_BEND_RANGE_MICROCENTS: u32 = 2_400_000_000;
pub const MAXIMUM_EVENT_TIME_MICROS: u64 = u64::MAX - 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SoundInfoError {
    WrongLength { expected: usize, actual: usize },
    OutOfRange(&'static str),
    InvalidTag { field: &'static str, actual: u8 },
    NonCanonicalReserved(&'static str),
    InconsistentPcmLength { expected: u32, actual: u32 },
}

/// Canonical physical pitch plus its explicit musical tuning relationship.
///
/// Frequency is milliHertz, so microtonal pitches are not forced through MIDI
/// key numbers. `detune_microcents` records transpose/fine detune relative to
/// the declared A4 reference without making equal temperament compulsory.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MusicalPitch {
    pub frequency_millihertz: u64,
    pub a4_reference_millihertz: u64,
    pub detune_microcents: i32,
}

impl MusicalPitch {
    pub fn new(
        frequency_millihertz: u64,
        a4_reference_millihertz: u64,
        detune_microcents: i32,
    ) -> Result<Self, SoundInfoError> {
        if !(MINIMUM_PITCH_MILLIHERTZ..=MAXIMUM_PITCH_MILLIHERTZ).contains(&frequency_millihertz) {
            return Err(SoundInfoError::OutOfRange("frequency-millihertz"));
        }
        if !(MINIMUM_A4_MILLIHERTZ..=MAXIMUM_A4_MILLIHERTZ).contains(&a4_reference_millihertz) {
            return Err(SoundInfoError::OutOfRange("a4-reference-millihertz"));
        }
        if detune_microcents.unsigned_abs() > MAXIMUM_ABSOLUTE_DETUNE_MICROCENTS as u32 {
            return Err(SoundInfoError::OutOfRange("detune-microcents"));
        }
        Ok(Self {
            frequency_millihertz,
            a4_reference_millihertz,
            detune_microcents,
        })
    }

    /// Constructs a twelve-tone equal-tempered pitch without exposing MIDI
    /// numbering. `semitones_from_a4` is unbounded by a protocol key range;
    /// the resulting physical frequency remains subject to this Info's exact
    /// audible range. Fine detune is expressed in microcents.
    pub fn from_equal_tempered(
        semitones_from_a4: i16,
        a4_reference_millihertz: u64,
        detune_microcents: i32,
    ) -> Result<Self, SoundInfoError> {
        if !(MINIMUM_A4_MILLIHERTZ..=MAXIMUM_A4_MILLIHERTZ).contains(&a4_reference_millihertz) {
            return Err(SoundInfoError::OutOfRange("a4-reference-millihertz"));
        }
        if detune_microcents.unsigned_abs() > MAXIMUM_ABSOLUTE_DETUNE_MICROCENTS as u32 {
            return Err(SoundInfoError::OutOfRange("detune-microcents"));
        }
        let semitone_microcents = i64::from(semitones_from_a4) * 100_000_000;
        let total_microcents = semitone_microcents + i64::from(detune_microcents);
        let octaves = total_microcents as f64 / 1_200_000_000.0;
        let frequency = libm::round(a4_reference_millihertz as f64 * libm::exp2(octaves));
        if !frequency.is_finite()
            || frequency < MINIMUM_PITCH_MILLIHERTZ as f64
            || frequency > MAXIMUM_PITCH_MILLIHERTZ as f64
        {
            return Err(SoundInfoError::OutOfRange("frequency-millihertz"));
        }
        Self::new(frequency as u64, a4_reference_millihertz, detune_microcents)
    }

    pub const fn encode(self) -> [u8; 20] {
        let mut out = [0; 20];
        let frequency = self.frequency_millihertz.to_le_bytes();
        let reference = self.a4_reference_millihertz.to_le_bytes();
        let detune = self.detune_microcents.to_le_bytes();
        let mut i = 0;
        while i < 8 {
            out[i] = frequency[i];
            out[8 + i] = reference[i];
            i += 1;
        }
        i = 0;
        while i < 4 {
            out[16 + i] = detune[i];
            i += 1;
        }
        out
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        exact_length(encoded, 20)?;
        Self::new(
            u64::from_le_bytes(array(encoded, 0)?),
            u64::from_le_bytes(array(encoded, 8)?),
            i32::from_le_bytes(array(encoded, 16)?),
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Gate {
    On,
    Off,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NoteOccurrenceId(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MusicalNoteEvent {
    pub occurrence: NoteOccurrenceId,
    pub pitch: MusicalPitch,
    pub gate: Gate,
    pub velocity: u16,
    pub event_time_micros: u64,
    pub order: u32,
}

impl MusicalNoteEvent {
    pub fn new(
        occurrence: NoteOccurrenceId,
        pitch: MusicalPitch,
        gate: Gate,
        velocity: u16,
        event_time_micros: u64,
        order: u32,
    ) -> Result<Self, SoundInfoError> {
        if occurrence.0 == 0 {
            return Err(SoundInfoError::OutOfRange("occurrence"));
        }
        if event_time_micros > MAXIMUM_EVENT_TIME_MICROS {
            return Err(SoundInfoError::OutOfRange("event-time-micros"));
        }
        Ok(Self {
            occurrence,
            pitch,
            gate,
            velocity,
            event_time_micros,
            order,
        })
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(MUSIC_NOTE_INFO_ID, &self.encode())
    }

    pub fn encode(self) -> [u8; NOTE_EVENT_ENCODED_LEN] {
        let mut out = [0; NOTE_EVENT_ENCODED_LEN];
        out[0..8].copy_from_slice(&self.occurrence.0.to_le_bytes());
        out[8..28].copy_from_slice(&self.pitch.encode());
        out[28] = match self.gate {
            Gate::On => 1,
            Gate::Off => 0,
        };
        out[29..31].copy_from_slice(&self.velocity.to_le_bytes());
        out[31..39].copy_from_slice(&self.event_time_micros.to_le_bytes());
        out[39..43].copy_from_slice(&self.order.to_le_bytes());
        out
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        exact_length(encoded, NOTE_EVENT_ENCODED_LEN)?;
        let gate = Gate::decode(encoded[28])?;
        Self::new(
            NoteOccurrenceId(u64::from_le_bytes(array(encoded, 0)?)),
            MusicalPitch::decode(&encoded[8..28])?,
            gate,
            u16::from_le_bytes(array(encoded, 29)?),
            u64::from_le_bytes(array(encoded, 31)?),
            u32::from_le_bytes(array(encoded, 39)?),
        )
    }
}

impl Gate {
    fn decode(value: u8) -> Result<Self, SoundInfoError> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            actual => Err(SoundInfoError::InvalidTag {
                field: "gate",
                actual,
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MusicalControl {
    Sustain {
        down: bool,
    },
    PitchBend {
        amount_millionths: i32,
        range_microcents: u32,
    },
    Modulation {
        amount_millionths: u32,
        destination: ModulationDestination,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ModulationDestination {
    Pitch,
    FilterCutoff,
    Amplitude,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MusicalControlEvent {
    pub control: MusicalControl,
    pub event_time_micros: u64,
    pub order: u32,
}

impl MusicalControlEvent {
    pub fn new(
        control: MusicalControl,
        event_time_micros: u64,
        order: u32,
    ) -> Result<Self, SoundInfoError> {
        if event_time_micros > MAXIMUM_EVENT_TIME_MICROS {
            return Err(SoundInfoError::OutOfRange("event-time-micros"));
        }
        match control {
            MusicalControl::PitchBend {
                amount_millionths,
                range_microcents,
            } if !(-1_000_000..=1_000_000).contains(&amount_millionths)
                || range_microcents > MAXIMUM_PITCH_BEND_RANGE_MICROCENTS =>
            {
                return Err(SoundInfoError::OutOfRange("pitch-bend"))
            }
            MusicalControl::Modulation {
                amount_millionths, ..
            } if amount_millionths > 1_000_000 => {
                return Err(SoundInfoError::OutOfRange("modulation"))
            }
            _ => {}
        }
        Ok(Self {
            control,
            event_time_micros,
            order,
        })
    }

    pub fn encode(self) -> [u8; CONTROL_EVENT_ENCODED_LEN] {
        let mut out = [0; CONTROL_EVENT_ENCODED_LEN];
        match self.control {
            MusicalControl::Sustain { down } => {
                out[0] = 0;
                out[1] = u8::from(down);
            }
            MusicalControl::PitchBend {
                amount_millionths,
                range_microcents,
            } => {
                out[0] = 1;
                out[1..5].copy_from_slice(&amount_millionths.to_le_bytes());
                out[5..9].copy_from_slice(&range_microcents.to_le_bytes());
            }
            MusicalControl::Modulation {
                amount_millionths,
                destination,
            } => {
                out[0] = 2;
                out[1..5].copy_from_slice(&amount_millionths.to_le_bytes());
                out[9] = destination.tag();
            }
        }
        out[10..18].copy_from_slice(&self.event_time_micros.to_le_bytes());
        out[18..22].copy_from_slice(&self.order.to_le_bytes());
        out
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(MUSIC_CONTROL_INFO_ID, &self.encode())
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        exact_length(encoded, CONTROL_EVENT_ENCODED_LEN)?;
        let control = match encoded[0] {
            0 => {
                require_zero(&encoded[2..10], "sustain-reserved")?;
                match encoded[1] {
                    0 => MusicalControl::Sustain { down: false },
                    1 => MusicalControl::Sustain { down: true },
                    actual => {
                        return Err(SoundInfoError::InvalidTag {
                            field: "sustain",
                            actual,
                        })
                    }
                }
            }
            1 => {
                require_zero(&encoded[9..10], "pitch-bend-reserved")?;
                MusicalControl::PitchBend {
                    amount_millionths: i32::from_le_bytes(array(encoded, 1)?),
                    range_microcents: u32::from_le_bytes(array(encoded, 5)?),
                }
            }
            2 => {
                require_zero(&encoded[5..9], "modulation-reserved")?;
                MusicalControl::Modulation {
                    amount_millionths: u32::from_le_bytes(array(encoded, 1)?),
                    destination: ModulationDestination::decode(encoded[9])?,
                }
            }
            actual => {
                return Err(SoundInfoError::InvalidTag {
                    field: "control",
                    actual,
                })
            }
        };
        Self::new(
            control,
            u64::from_le_bytes(array(encoded, 10)?),
            u32::from_le_bytes(array(encoded, 18)?),
        )
    }
}

impl ModulationDestination {
    const fn tag(self) -> u8 {
        match self {
            Self::Pitch => 0,
            Self::FilterCutoff => 1,
            Self::Amplitude => 2,
        }
    }
    fn decode(actual: u8) -> Result<Self, SoundInfoError> {
        match actual {
            0 => Ok(Self::Pitch),
            1 => Ok(Self::FilterCutoff),
            2 => Ok(Self::Amplitude),
            actual => Err(SoundInfoError::InvalidTag {
                field: "modulation-destination",
                actual,
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ToneIntent {
    pub correlation: u64,
    pub pitch: MusicalPitch,
    pub gate: Gate,
    pub event_time_micros: u64,
    pub order: u32,
}

impl ToneIntent {
    pub fn new(
        correlation: u64,
        pitch: MusicalPitch,
        gate: Gate,
        event_time_micros: u64,
        order: u32,
    ) -> Result<Self, SoundInfoError> {
        if correlation == 0 {
            return Err(SoundInfoError::OutOfRange("correlation"));
        }
        if event_time_micros > MAXIMUM_EVENT_TIME_MICROS {
            return Err(SoundInfoError::OutOfRange("event-time-micros"));
        }
        Ok(Self {
            correlation,
            pitch,
            gate,
            event_time_micros,
            order,
        })
    }

    pub fn encode(self) -> [u8; TONE_INTENT_ENCODED_LEN] {
        let mut out = [0; TONE_INTENT_ENCODED_LEN];
        out[0..8].copy_from_slice(&self.correlation.to_le_bytes());
        out[8..28].copy_from_slice(&self.pitch.encode());
        out[28] = match self.gate {
            Gate::Off => 0,
            Gate::On => 1,
        };
        out[29..37].copy_from_slice(&self.event_time_micros.to_le_bytes());
        out[37..41].copy_from_slice(&self.order.to_le_bytes());
        out
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(SOUND_TONE_INFO_ID, &self.encode())
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        exact_length(encoded, TONE_INTENT_ENCODED_LEN)?;
        Self::new(
            u64::from_le_bytes(array(encoded, 0)?),
            MusicalPitch::decode(&encoded[8..28])?,
            Gate::decode(encoded[28])?,
            u64::from_le_bytes(array(encoded, 29)?),
            u32::from_le_bytes(array(encoded, 37)?),
        )
    }
}

fn exact_length(encoded: &[u8], expected: usize) -> Result<(), SoundInfoError> {
    if encoded.len() != expected {
        return Err(SoundInfoError::WrongLength {
            expected,
            actual: encoded.len(),
        });
    }
    Ok(())
}

fn array<const N: usize>(encoded: &[u8], start: usize) -> Result<[u8; N], SoundInfoError> {
    encoded
        .get(start..start + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(SoundInfoError::WrongLength {
            expected: start + N,
            actual: encoded.len(),
        })
}

fn require_zero(encoded: &[u8], field: &'static str) -> Result<(), SoundInfoError> {
    if encoded.iter().any(|byte| *byte != 0) {
        return Err(SoundInfoError::NonCanonicalReserved(field));
    }
    Ok(())
}
