use conduit_core::{Gate, ModulationDestination, MusicalControl, MusicalPitch, NoteOccurrenceId};
use conduit_midi::{
    midi_velocity_to_portable, MidiAdapterError, MidiInputAdapter, MidiMessage, MidiOutputAdapter,
    MidiParseError, MidiParser, MidiProfile, ParsedMidi, PortableMidiEvent,
};

fn profile() -> MidiProfile {
    MidiProfile::new(440_000, Some(0), 0).unwrap()
}

fn message(parser: &mut MidiParser, bytes: &[u8]) -> MidiMessage {
    let mut result = None;
    for byte in bytes {
        if let Some(ParsedMidi::Message(parsed)) = parser.feed(*byte).unwrap() {
            result = Some(parsed);
        }
    }
    result.unwrap()
}

#[test]
fn canonical_messages_running_status_and_velocity_zero_are_exact() {
    let mut parser = MidiParser::new();
    assert_eq!(
        message(&mut parser, &[0x90, 60, 100]),
        MidiMessage::NoteOn {
            channel: 0,
            key: 60,
            velocity: 100
        }
    );
    assert_eq!(
        message(&mut parser, &[61, 0]),
        MidiMessage::NoteOn {
            channel: 0,
            key: 61,
            velocity: 0
        }
    );
    let mut adapter = MidiInputAdapter::new(profile(), 1).unwrap();
    let on = adapter
        .accept(
            MidiMessage::NoteOn {
                channel: 0,
                key: 61,
                velocity: 90,
            },
            10,
        )
        .unwrap();
    let off = adapter
        .accept(
            MidiMessage::NoteOn {
                channel: 0,
                key: 61,
                velocity: 0,
            },
            20,
        )
        .unwrap();
    let (PortableMidiEvent::Note(on), PortableMidiEvent::Note(off)) = (on, off) else {
        panic!("note messages must produce note Info")
    };
    assert_eq!(on.gate, Gate::On);
    assert_eq!(off.gate, Gate::Off);
    assert_eq!(on.occurrence, off.occurrence);
    assert_eq!(on.velocity, midi_velocity_to_portable(90));
}

#[test]
fn tuning_and_transpose_become_exact_portable_pitch() {
    let profile = MidiProfile::new(442_000, Some(0), 0)
        .unwrap()
        .with_transpose(12)
        .unwrap();
    let mut adapter = MidiInputAdapter::new(profile, 1).unwrap();
    let PortableMidiEvent::Note(note) = adapter
        .accept(
            MidiMessage::NoteOn {
                channel: 0,
                key: 69,
                velocity: 100,
            },
            10,
        )
        .unwrap()
    else {
        panic!("note-on must produce portable note Info")
    };
    assert_eq!(note.pitch.a4_reference_millihertz, 442_000);
    assert_eq!(note.pitch.frequency_millihertz, 884_000);
    assert_eq!(
        MidiProfile::new(440_000, None, 0)
            .unwrap()
            .with_transpose(49),
        Err(MidiAdapterError::TransposeOutOfRange)
    );
}

#[test]
fn overlapping_equal_keys_pair_last_on_first_off_deterministically() {
    let mut adapter = MidiInputAdapter::new(profile(), 40).unwrap();
    let first = adapter
        .accept(
            MidiMessage::NoteOn {
                channel: 0,
                key: 69,
                velocity: 64,
            },
            1,
        )
        .unwrap();
    let second = adapter
        .accept(
            MidiMessage::NoteOn {
                channel: 0,
                key: 69,
                velocity: 65,
            },
            2,
        )
        .unwrap();
    let first_off = adapter
        .accept(
            MidiMessage::NoteOff {
                channel: 0,
                key: 69,
                velocity: 0,
            },
            3,
        )
        .unwrap();
    let second_off = adapter
        .accept(
            MidiMessage::NoteOff {
                channel: 0,
                key: 69,
                velocity: 0,
            },
            4,
        )
        .unwrap();
    let occurrence = |event| match event {
        PortableMidiEvent::Note(note) => note.occurrence,
        _ => panic!("expected note"),
    };
    assert_eq!(occurrence(first), NoteOccurrenceId(40));
    assert_eq!(occurrence(second), NoteOccurrenceId(41));
    assert_eq!(occurrence(first_off), NoteOccurrenceId(41));
    assert_eq!(occurrence(second_off), NoteOccurrenceId(40));
}

#[test]
fn sustain_modulation_and_pitch_bend_become_typed_controls() {
    let mut adapter = MidiInputAdapter::new(profile(), 1).unwrap();
    let sustain = adapter
        .accept(
            MidiMessage::ControlChange {
                channel: 0,
                controller: 64,
                value: 127,
            },
            1,
        )
        .unwrap();
    let modulation = adapter
        .accept(
            MidiMessage::ControlChange {
                channel: 0,
                controller: 1,
                value: 127,
            },
            2,
        )
        .unwrap();
    let bend = adapter
        .accept(
            MidiMessage::PitchBend {
                channel: 0,
                value: 8192,
            },
            3,
        )
        .unwrap();
    let bend_minimum = adapter
        .accept(
            MidiMessage::PitchBend {
                channel: 0,
                value: 0,
            },
            4,
        )
        .unwrap();
    let bend_maximum = adapter
        .accept(
            MidiMessage::PitchBend {
                channel: 0,
                value: 16_383,
            },
            5,
        )
        .unwrap();
    assert!(matches!(
        sustain,
        PortableMidiEvent::Control(event)
            if event.control == MusicalControl::Sustain { down: true }
    ));
    assert!(matches!(
        modulation,
        PortableMidiEvent::Control(event)
            if event.control == MusicalControl::Modulation {
                amount_millionths: 1_000_000,
                destination: ModulationDestination::Pitch,
            }
    ));
    assert!(matches!(
        bend,
        PortableMidiEvent::Control(event)
            if event.control == MusicalControl::PitchBend {
                amount_millionths: 0,
                range_microcents: 200_000_000,
            }
    ));
    assert!(matches!(
        bend_minimum,
        PortableMidiEvent::Control(event)
            if matches!(event.control, MusicalControl::PitchBend { amount_millionths: -1_000_000, .. })
    ));
    assert!(matches!(
        bend_maximum,
        PortableMidiEvent::Control(event)
            if matches!(event.control, MusicalControl::PitchBend { amount_millionths: 1_000_000, .. })
    ));
}

#[test]
fn parser_refuses_truncation_unframed_data_and_oversize_sysex() {
    let mut parser = MidiParser::new();
    assert_eq!(parser.feed(7), Err(MidiParseError::UnexpectedData(7)));
    parser.feed(0x90).unwrap();
    parser.feed(60).unwrap();
    assert_eq!(parser.finish(), Err(MidiParseError::DataByteExpected(1)));
    parser.feed(0x90).unwrap();
    parser.feed(60).unwrap();
    assert_eq!(parser.feed(0x80), Err(MidiParseError::DataByteExpected(1)));
    parser.feed(0xf0).unwrap();
    for _ in 1..256 {
        parser.feed(1).unwrap();
    }
    assert_eq!(parser.feed(1), Err(MidiParseError::SysExCapacityExceeded));

    parser.feed(0xf0).unwrap();
    for _ in 1..256 {
        parser.feed(1).unwrap();
    }
    assert_eq!(
        parser.feed(0xf7),
        Err(MidiParseError::SysExCapacityExceeded)
    );
}

#[test]
fn output_requires_exact_midi_profile_and_clears_notes_on_cancel() {
    let mut output = MidiOutputAdapter::new(profile());
    let pitch = MusicalPitch::from_equal_tempered(0, 440_000, 0).unwrap();
    let on = conduit_core::MusicalNoteEvent::new(
        NoteOccurrenceId(7),
        pitch,
        Gate::On,
        midi_velocity_to_portable(100),
        1,
        0,
    )
    .unwrap();
    assert_eq!(output.encode_note(on).unwrap(), [0x90, 69, 100]);
    assert_eq!(output.cancel_all_notes_off(), [0xb0, 123, 0]);
    let microtonal = conduit_core::MusicalNoteEvent::new(
        NoteOccurrenceId(8),
        MusicalPitch::from_equal_tempered(0, 440_000, 1).unwrap(),
        Gate::On,
        midi_velocity_to_portable(100),
        2,
        1,
    )
    .unwrap();
    assert_eq!(
        output.encode_note(microtonal),
        Err(MidiAdapterError::PitchOutsideMidiProfile)
    );
}
