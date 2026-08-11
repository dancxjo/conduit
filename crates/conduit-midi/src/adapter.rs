use conduit_core::{
    Gate, ModulationDestination, MusicalControl, MusicalControlEvent, MusicalNoteEvent,
    MusicalPitch, NoteOccurrenceId,
};

use crate::MidiMessage;

pub const MAXIMUM_ACTIVE_NOTES: usize = 128;
pub const MIDI_PITCH_BEND_RANGE_MICROCENTS: u32 = 200_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiProfile {
    pub a4_reference_millihertz: u64,
    pub transpose_semitones: i16,
    pub input_channel: Option<u8>,
    pub output_channel: u8,
}

impl MidiProfile {
    pub fn new(
        a4_reference_millihertz: u64,
        input_channel: Option<u8>,
        output_channel: u8,
    ) -> Result<Self, MidiAdapterError> {
        if input_channel.is_some_and(|channel| channel > 15) || output_channel > 15 {
            return Err(MidiAdapterError::ChannelOutOfRange);
        }
        MusicalPitch::from_equal_tempered(0, a4_reference_millihertz, 0)
            .map_err(|_| MidiAdapterError::TuningUnsupported)?;
        Ok(Self {
            a4_reference_millihertz,
            transpose_semitones: 0,
            input_channel,
            output_channel,
        })
    }

    pub fn with_transpose(mut self, transpose_semitones: i16) -> Result<Self, MidiAdapterError> {
        if !(-48..=48).contains(&transpose_semitones) {
            return Err(MidiAdapterError::TransposeOutOfRange);
        }
        self.transpose_semitones = transpose_semitones;
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortableMidiEvent {
    Note(MusicalNoteEvent),
    Control(MusicalControlEvent),
    UnsupportedControl { controller: u8 },
    IgnoredChannel { channel: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiAdapterError {
    ChannelOutOfRange,
    TransposeOutOfRange,
    TuningUnsupported,
    ActiveNoteCapacityExceeded,
    NoteOffWithoutActiveOccurrence,
    OccurrenceExhausted,
    DuplicateOccurrence,
    PitchOutsideMidiProfile,
    VelocityOutsideMidiProfile,
    ControlOutsideMidiProfile,
    PitchBendOutsideMidiProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveNote {
    occurrence: NoteOccurrenceId,
    channel: u8,
    key: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputNote {
    occurrence: NoteOccurrenceId,
    key: u8,
}

pub struct MidiInputAdapter {
    profile: MidiProfile,
    active: [Option<ActiveNote>; MAXIMUM_ACTIVE_NOTES],
    next_occurrence: u64,
    next_order: u32,
}

impl MidiInputAdapter {
    pub fn new(profile: MidiProfile, first_occurrence: u64) -> Result<Self, MidiAdapterError> {
        if first_occurrence == 0 {
            return Err(MidiAdapterError::OccurrenceExhausted);
        }
        Ok(Self {
            profile,
            active: [None; MAXIMUM_ACTIVE_NOTES],
            next_occurrence: first_occurrence,
            next_order: 0,
        })
    }

    pub fn accept(
        &mut self,
        message: MidiMessage,
        event_time_micros: u64,
    ) -> Result<PortableMidiEvent, MidiAdapterError> {
        let channel = message_channel(message);
        if self
            .profile
            .input_channel
            .is_some_and(|accepted| accepted != channel)
        {
            return Ok(PortableMidiEvent::IgnoredChannel { channel });
        }
        match message {
            MidiMessage::NoteOn {
                channel,
                key,
                velocity: 0,
            }
            | MidiMessage::NoteOff {
                channel,
                key,
                velocity: _,
            } => self.note_off(channel, key, event_time_micros),
            MidiMessage::NoteOn {
                channel,
                key,
                velocity,
            } => self.note_on(channel, key, velocity, event_time_micros),
            MidiMessage::ControlChange {
                controller: 64,
                value,
                ..
            } => self.control(
                MusicalControl::Sustain { down: value >= 64 },
                event_time_micros,
            ),
            MidiMessage::ControlChange {
                controller: 1,
                value,
                ..
            } => self.control(
                MusicalControl::Modulation {
                    amount_millionths: seven_bit_to_millionths(value),
                    destination: ModulationDestination::Pitch,
                },
                event_time_micros,
            ),
            MidiMessage::ControlChange { controller, .. } => {
                Ok(PortableMidiEvent::UnsupportedControl { controller })
            }
            MidiMessage::PitchBend { value, .. } => self.control(
                MusicalControl::PitchBend {
                    amount_millionths: pitch_bend_to_millionths(value),
                    range_microcents: MIDI_PITCH_BEND_RANGE_MICROCENTS,
                },
                event_time_micros,
            ),
            MidiMessage::UnsupportedChannel { .. } => {
                Ok(PortableMidiEvent::UnsupportedControl { controller: 0xff })
            }
        }
    }

    pub fn cancel(&mut self) {
        self.active.fill(None);
    }

    pub fn active_notes(&self) -> usize {
        self.active.iter().flatten().count()
    }

    fn note_on(
        &mut self,
        channel: u8,
        key: u8,
        velocity: u8,
        event_time_micros: u64,
    ) -> Result<PortableMidiEvent, MidiAdapterError> {
        let occurrence = NoteOccurrenceId(self.next_occurrence);
        self.next_occurrence = self
            .next_occurrence
            .checked_add(1)
            .ok_or(MidiAdapterError::OccurrenceExhausted)?;
        let slot = self
            .active
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(MidiAdapterError::ActiveNoteCapacityExceeded)?;
        *slot = Some(ActiveNote {
            occurrence,
            channel,
            key,
        });
        self.note_event(
            occurrence,
            key,
            Gate::On,
            midi_velocity_to_portable(velocity),
            event_time_micros,
        )
    }

    fn note_off(
        &mut self,
        channel: u8,
        key: u8,
        event_time_micros: u64,
    ) -> Result<PortableMidiEvent, MidiAdapterError> {
        let index = self
            .active
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                note.filter(|note| note.channel == channel && note.key == key)
                    .map(|note| (index, note.occurrence.0))
            })
            .max_by_key(|(_, occurrence)| *occurrence)
            .map(|(index, _)| index)
            .ok_or(MidiAdapterError::NoteOffWithoutActiveOccurrence)?;
        let note = self.active[index]
            .take()
            .ok_or(MidiAdapterError::NoteOffWithoutActiveOccurrence)?;
        self.note_event(note.occurrence, key, Gate::Off, 0, event_time_micros)
    }

    fn note_event(
        &mut self,
        occurrence: NoteOccurrenceId,
        key: u8,
        gate: Gate,
        velocity: u16,
        event_time_micros: u64,
    ) -> Result<PortableMidiEvent, MidiAdapterError> {
        let pitch = MusicalPitch::from_equal_tempered(
            i16::from(key) - 69 + self.profile.transpose_semitones,
            self.profile.a4_reference_millihertz,
            0,
        )
        .map_err(|_| MidiAdapterError::PitchOutsideMidiProfile)?;
        let order = self.take_order()?;
        MusicalNoteEvent::new(occurrence, pitch, gate, velocity, event_time_micros, order)
            .map(PortableMidiEvent::Note)
            .map_err(|_| MidiAdapterError::PitchOutsideMidiProfile)
    }

    fn control(
        &mut self,
        control: MusicalControl,
        event_time_micros: u64,
    ) -> Result<PortableMidiEvent, MidiAdapterError> {
        let order = self.take_order()?;
        MusicalControlEvent::new(control, event_time_micros, order)
            .map(PortableMidiEvent::Control)
            .map_err(|_| MidiAdapterError::ControlOutsideMidiProfile)
    }

    fn take_order(&mut self) -> Result<u32, MidiAdapterError> {
        let order = self.next_order;
        self.next_order = self
            .next_order
            .checked_add(1)
            .ok_or(MidiAdapterError::OccurrenceExhausted)?;
        Ok(order)
    }
}

pub struct MidiOutputAdapter {
    profile: MidiProfile,
    active: [Option<OutputNote>; MAXIMUM_ACTIVE_NOTES],
}

impl MidiOutputAdapter {
    pub const fn new(profile: MidiProfile) -> Self {
        Self {
            profile,
            active: [None; MAXIMUM_ACTIVE_NOTES],
        }
    }

    pub fn encode_note(&mut self, event: MusicalNoteEvent) -> Result<[u8; 3], MidiAdapterError> {
        match event.gate {
            Gate::On => {
                if self
                    .active
                    .iter()
                    .flatten()
                    .any(|note| note.occurrence == event.occurrence)
                {
                    return Err(MidiAdapterError::DuplicateOccurrence);
                }
                let key = exact_midi_key(event.pitch, self.profile.a4_reference_millihertz)?;
                let velocity = exact_midi_velocity(event.velocity)?;
                let slot = self
                    .active
                    .iter_mut()
                    .find(|slot| slot.is_none())
                    .ok_or(MidiAdapterError::ActiveNoteCapacityExceeded)?;
                *slot = Some(OutputNote {
                    occurrence: event.occurrence,
                    key,
                });
                Ok([0x90 | self.profile.output_channel, key, velocity])
            }
            Gate::Off => {
                let slot = self
                    .active
                    .iter_mut()
                    .find(|slot| slot.is_some_and(|note| note.occurrence == event.occurrence))
                    .ok_or(MidiAdapterError::NoteOffWithoutActiveOccurrence)?;
                let note = slot
                    .take()
                    .ok_or(MidiAdapterError::NoteOffWithoutActiveOccurrence)?;
                Ok([0x80 | self.profile.output_channel, note.key, 0])
            }
        }
    }

    pub fn encode_control(&self, event: MusicalControlEvent) -> Result<[u8; 3], MidiAdapterError> {
        match event.control {
            MusicalControl::Sustain { down } => Ok([
                0xb0 | self.profile.output_channel,
                64,
                if down { 127 } else { 0 },
            ]),
            MusicalControl::Modulation {
                amount_millionths,
                destination: ModulationDestination::Pitch,
            } => Ok([
                0xb0 | self.profile.output_channel,
                1,
                exact_seven_bit(amount_millionths)?,
            ]),
            MusicalControl::PitchBend {
                amount_millionths,
                range_microcents: MIDI_PITCH_BEND_RANGE_MICROCENTS,
            } => {
                let value = exact_pitch_bend(amount_millionths)?;
                Ok([
                    0xe0 | self.profile.output_channel,
                    (value & 0x7f) as u8,
                    (value >> 7) as u8,
                ])
            }
            _ => Err(MidiAdapterError::ControlOutsideMidiProfile),
        }
    }

    pub fn cancel_all_notes_off(&mut self) -> [u8; 3] {
        self.active.fill(None);
        [0xb0 | self.profile.output_channel, 123, 0]
    }
}

pub const fn midi_velocity_to_portable(value: u8) -> u16 {
    ((value as u32 * u16::MAX as u32) / 127) as u16
}

const fn seven_bit_to_millionths(value: u8) -> u32 {
    value as u32 * 1_000_000 / 127
}

const fn pitch_bend_to_millionths(value: u16) -> i32 {
    if value >= 8192 {
        (((value - 8192) as i64 * 1_000_000) / 8191) as i32
    } else {
        -((((8192 - value) as i64 * 1_000_000) / 8192) as i32)
    }
}

fn exact_midi_velocity(value: u16) -> Result<u8, MidiAdapterError> {
    (0_u8..=127)
        .find(|candidate| midi_velocity_to_portable(*candidate) == value)
        .ok_or(MidiAdapterError::VelocityOutsideMidiProfile)
}

fn exact_seven_bit(value: u32) -> Result<u8, MidiAdapterError> {
    (0_u8..=127)
        .find(|candidate| seven_bit_to_millionths(*candidate) == value)
        .ok_or(MidiAdapterError::ControlOutsideMidiProfile)
}

fn exact_pitch_bend(amount: i32) -> Result<u16, MidiAdapterError> {
    (0_u16..=16_383)
        .find(|candidate| pitch_bend_to_millionths(*candidate) == amount)
        .ok_or(MidiAdapterError::PitchBendOutsideMidiProfile)
}

fn exact_midi_key(pitch: MusicalPitch, reference: u64) -> Result<u8, MidiAdapterError> {
    if pitch.a4_reference_millihertz != reference || pitch.detune_microcents != 0 {
        return Err(MidiAdapterError::PitchOutsideMidiProfile);
    }
    (0_u8..=127)
        .find(|key| {
            MusicalPitch::from_equal_tempered(i16::from(*key) - 69, reference, 0)
                .is_ok_and(|candidate| candidate == pitch)
        })
        .ok_or(MidiAdapterError::PitchOutsideMidiProfile)
}

const fn message_channel(message: MidiMessage) -> u8 {
    match message {
        MidiMessage::NoteOff { channel, .. }
        | MidiMessage::NoteOn { channel, .. }
        | MidiMessage::ControlChange { channel, .. }
        | MidiMessage::PitchBend { channel, .. } => channel,
        MidiMessage::UnsupportedChannel { status, .. } => status & 0x0f,
    }
}
