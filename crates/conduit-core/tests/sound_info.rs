use conduit_core::{
    AudioRenderDemand, Gate, ModulationDestination, MusicalControl, MusicalControlEvent,
    MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, Quantity, QuantityConversionRefusal,
    QuantityUnit, SoundInfoError, ToneIntent,
};

#[test]
fn render_demand_round_trips_one_exact_nonempty_clock_interval() {
    let demand = AudioRenderDemand::new(7, 480, 240, 2).unwrap();
    assert_eq!(AudioRenderDemand::decode(&demand.encode()), Ok(demand));
    assert_ne!(demand.semantic_digest(), [0; 32]);
    assert_eq!(
        AudioRenderDemand::new(0, 0, 240, 0),
        Err(SoundInfoError::OutOfRange("render-clock-id"))
    );
    assert_eq!(
        AudioRenderDemand::new(1, 0, 0, 0),
        Err(SoundInfoError::OutOfRange("render-frame-count"))
    );
}

#[test]
fn overlapping_equal_pitches_retain_occurrence_identity() {
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let a = MusicalNoteEvent::new(NoteOccurrenceId(1), pitch, Gate::On, 32_768, 10, 0).unwrap();
    let b = MusicalNoteEvent::new(NoteOccurrenceId(2), pitch, Gate::On, 32_768, 10, 1).unwrap();
    assert_ne!(a.semantic_digest(), b.semantic_digest());
}

#[test]
fn sustain_hold_and_release_are_distinct_ordered_events() {
    let pitch = MusicalPitch::new(261_626, 440_000, 0).unwrap();
    let occurrence = NoteOccurrenceId(7);
    let sequence = [
        MusicalNoteEvent::new(occurrence, pitch, Gate::On, 32_768, 1_000, 0)
            .unwrap()
            .encode()
            .to_vec(),
        MusicalControlEvent::new(MusicalControl::Sustain { down: true }, 1_100, 1)
            .unwrap()
            .encode()
            .to_vec(),
        MusicalNoteEvent::new(occurrence, pitch, Gate::Off, 0, 1_200, 2)
            .unwrap()
            .encode()
            .to_vec(),
        MusicalControlEvent::new(MusicalControl::Sustain { down: false }, 1_300, 3)
            .unwrap()
            .encode()
            .to_vec(),
    ];

    assert_eq!(MusicalNoteEvent::decode(&sequence[0]).unwrap().order, 0);
    assert_eq!(
        MusicalControlEvent::decode(&sequence[1]).unwrap().control,
        MusicalControl::Sustain { down: true }
    );
    assert_eq!(MusicalNoteEvent::decode(&sequence[2]).unwrap().order, 2);
    assert_eq!(
        MusicalControlEvent::decode(&sequence[3]).unwrap().control,
        MusicalControl::Sustain { down: false }
    );
}

#[test]
fn microtonal_pitch_and_explicit_tuning_are_not_midi_numbers() {
    let pitch = MusicalPitch::new(440_127, 442_000, 12_500).unwrap();
    assert_eq!(pitch.frequency_millihertz, 440_127);
    assert_eq!(pitch.a4_reference_millihertz, 442_000);
    let a4 = MusicalPitch::from_equal_tempered(0, 442_000, 0).unwrap();
    let a5 = MusicalPitch::from_equal_tempered(12, 442_000, 0).unwrap();
    let c4 = MusicalPitch::from_equal_tempered(-9, 440_000, 0).unwrap();
    assert_eq!(a4.frequency_millihertz, 442_000);
    assert_eq!(a5.frequency_millihertz, 884_000);
    assert_eq!(c4.frequency_millihertz, 261_626);
    assert_eq!(
        MusicalPitch::from_equal_tempered(0, 440_000, 50_000_000),
        Err(SoundInfoError::OutOfRange("detune-microcents"))
    );
}

#[test]
fn pitch_consumes_typed_frequency_without_changing_canonical_identity() {
    let legacy = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let typed = MusicalPitch::from_quantities(
        Quantity::new(440, QuantityUnit::Hertz),
        Quantity::new(440_000, QuantityUnit::Millihertz),
        0,
    )
    .unwrap();
    assert_eq!(typed, legacy);
    assert_eq!(typed.encode(), legacy.encode());
    assert_eq!(
        typed.frequency(),
        Quantity::new(440_000, QuantityUnit::Millihertz)
    );
    assert_eq!(
        MusicalPitch::from_quantities(
            Quantity::new(1, QuantityUnit::Millisecond),
            Quantity::new(440, QuantityUnit::Hertz),
            0,
        ),
        Err(SoundInfoError::QuantityConversion(
            QuantityConversionRefusal::IncompatibleDimensions
        ))
    );
}

#[test]
fn every_portable_event_round_trips_and_reserved_bytes_refuse() {
    let pitch = MusicalPitch::new(261_626, 440_000, -7_500).unwrap();
    let tone = ToneIntent::new(9, pitch, Gate::On, 12, 3).unwrap();
    assert_eq!(ToneIntent::decode(&tone.encode()), Ok(tone));
    let note =
        MusicalNoteEvent::new(NoteOccurrenceId(11), pitch, Gate::Off, 65_535, 13, 4).unwrap();
    assert_eq!(MusicalNoteEvent::decode(&note.encode()), Ok(note));
    for control in [
        MusicalControl::Sustain { down: true },
        MusicalControl::PitchBend {
            amount_millionths: -500_000,
            range_microcents: 200_000_000,
        },
        MusicalControl::Modulation {
            amount_millionths: 750_000,
            destination: ModulationDestination::FilterCutoff,
        },
    ] {
        let event = MusicalControlEvent::new(control, 14, 5).unwrap();
        assert_eq!(MusicalControlEvent::decode(&event.encode()), Ok(event));
        assert_ne!(event.semantic_digest(), [0; 32]);
    }
    let mut noncanonical = MusicalControlEvent::new(MusicalControl::Sustain { down: false }, 0, 0)
        .unwrap()
        .encode();
    noncanonical[5] = 1;
    assert_eq!(
        MusicalControlEvent::decode(&noncanonical),
        Err(SoundInfoError::NonCanonicalReserved("sustain-reserved"))
    );
    assert_ne!(tone.semantic_digest(), [0; 32]);
}

#[test]
fn velocity_and_pitch_bend_cover_their_exact_extrema() {
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    for velocity in [0, u16::MAX / 2, u16::MAX] {
        let note = MusicalNoteEvent::new(
            NoteOccurrenceId(u64::from(velocity) + 1),
            pitch,
            Gate::On,
            velocity,
            0,
            u32::from(velocity),
        )
        .unwrap();
        assert_eq!(MusicalNoteEvent::decode(&note.encode()), Ok(note));
    }
    for amount in [-1_000_000, 0, 1_000_000] {
        let bend = MusicalControlEvent::new(
            MusicalControl::PitchBend {
                amount_millionths: amount,
                range_microcents: 200_000_000,
            },
            0,
            0,
        )
        .unwrap();
        assert_eq!(MusicalControlEvent::decode(&bend.encode()), Ok(bend));
    }
    assert_eq!(
        MusicalControlEvent::new(
            MusicalControl::PitchBend {
                amount_millionths: 1_000_001,
                range_microcents: 200_000_000,
            },
            0,
            0,
        ),
        Err(SoundInfoError::OutOfRange("pitch-bend"))
    );
}
