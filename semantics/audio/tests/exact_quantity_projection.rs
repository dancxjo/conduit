use conduit_audio::{Gate, MusicalPitch, SoundInfoError, ToneIntent};
use conduit_core::{ExactDecimalQuantity, QuantityConversionRefusal as R, QuantityUnit};
use conduit_plot::rust_binding::NativeRustBinding;

fn quantity(source: &str) -> ExactDecimalQuantity {
    let admitted = ExactDecimalQuantity::parse_plot_literal(source).unwrap();
    // The consumer receives the checked versioned transport, not a unit name
    // inferred from its field or an unchecked decimal cast.
    ExactDecimalQuantity::decode(&admitted.encode()).unwrap()
}

#[test]
fn prefixed_pitch_and_time_retain_existing_native_records() {
    let expected_pitch = MusicalPitch::new(440_127, 440_000, 12_500).unwrap();
    for source in ["440.127Hz", "0.440127kHz", "440127000µHz", "440127000uHz"] {
        let pitch =
            MusicalPitch::from_exact_quantities(quantity(source), quantity("0.44kHz"), 12_500)
                .unwrap();
        assert_eq!(pitch, expected_pitch);
        assert_eq!(pitch.encode(), expected_pitch.encode());
        assert_eq!(
            MusicalPitch::from_structured(pitch.into_structured().unwrap()).unwrap(),
            pitch
        );
        for time in ["250ms", "0.25s", "250000000ns", "250000µs"] {
            let tone = ToneIntent::from_exact_time(9, pitch, Gate::On, quantity(time), 3).unwrap();
            let expected = ToneIntent::new(9, expected_pitch, Gate::On, 250_000, 3).unwrap();
            assert_eq!(tone.encode(), expected.encode());
            assert_eq!(tone.semantic_digest(), expected.semantic_digest());
            assert_eq!(
                ToneIntent::from_structured(tone.into_structured().unwrap()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn sound_projection_refuses_precision_dimension_and_range_without_rounding() {
    for (source, expected) in [
        ("440.0001Hz", SoundInfoError::QuantityConversion(R::Inexact)),
        (
            "440ms",
            SoundInfoError::QuantityConversion(R::IncompatibleDimensions),
        ),
        ("1QHz", SoundInfoError::QuantityConversion(R::Overflow)),
        ("40.001kHz", SoundInfoError::OutOfRange("musical-pitch")),
    ] {
        assert_eq!(
            MusicalPitch::from_exact_quantities(quantity(source), quantity("440Hz"), 0),
            Err(expected)
        );
    }
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    for (source, expected) in [
        ("1ns", SoundInfoError::QuantityConversion(R::Inexact)),
        (
            "1Hz",
            SoundInfoError::QuantityConversion(R::IncompatibleDimensions),
        ),
        ("-1µs", SoundInfoError::QuantityConversion(R::Overflow)),
        ("1Qs", SoundInfoError::QuantityConversion(R::Overflow)),
    ] {
        assert_eq!(
            ToneIntent::from_exact_time(9, pitch, Gate::On, quantity(source), 0),
            Err(expected)
        );
    }
}

#[test]
fn unsigned_time_projection_preserves_the_full_existing_event_range() {
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let last = quantity("18446744073709.551614s");
    assert_eq!(
        last.convert_to_u64(QuantityUnit::Microsecond),
        Ok(u64::MAX - 1)
    );
    let tone = ToneIntent::from_exact_time(9, pitch, Gate::Off, last, u32::MAX).unwrap();
    assert_eq!(
        tone,
        ToneIntent::new(9, pitch, Gate::Off, u64::MAX - 1, u32::MAX).unwrap()
    );
    let reserved = quantity("18446744073709.551615s");
    assert_eq!(
        reserved.convert_to_u64(QuantityUnit::Microsecond),
        Ok(u64::MAX)
    );
    assert_eq!(
        ToneIntent::from_exact_time(9, pitch, Gate::Off, reserved, 0),
        Err(SoundInfoError::OutOfRange("tone-intent"))
    );
    assert_eq!(
        quantity("18446744073709.551616s").convert_to_u64(QuantityUnit::Microsecond),
        Err(R::Overflow)
    );
    assert_eq!(
        quantity("0qs").convert_to_u64(QuantityUnit::Microsecond),
        Ok(0)
    );
    assert_eq!(
        quantity("10000000000000s").convert_to_u64(QuantityUnit::Microsecond),
        Ok(10_000_000_000_000_000_000)
    );
}
