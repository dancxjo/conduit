use conduit_audio::*;
use conduit_core::{ExactNumericWork, Quantity};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn exact_literals_keep_authored_custody_and_transport_admitted_audio_values() {
    for literal in ["440Hz", "0.5kHz", "1.25Hz"] {
        let source = Quantity::parse_plot_literal(literal).unwrap();
        let value = prepare_exact_audio_frequency(
            source,
            literal,
            &mut ExactNumericWork::new(1_000_000_000),
        )
        .unwrap();
        assert_eq!(value.receipt().original(), literal);
        assert_eq!(value.receipt().source(), source);
        assert_eq!(
            AudioFrequencyHz::decode(value.canonical()).unwrap(),
            *value.value()
        );
    }
    let literal = "250ms";
    let value = prepare_exact_audio_time(
        Quantity::parse_plot_literal(literal).unwrap(),
        literal,
        &mut ExactNumericWork::new(1_000_000_000),
    )
    .unwrap();
    assert_eq!(*value.value().numerator_seconds(), 1);
    assert_eq!(*value.value().denominator(), 4);
    assert_eq!(value.receipt().original(), literal);
    assert_eq!(
        AudioExactTimeOffset::decode(value.canonical()).unwrap(),
        *value.value()
    );
}

#[test]
fn foreign_dimensions_negative_frequency_stale_spelling_and_exhausted_work_refuse() {
    for literal in ["1m", "-440Hz", "0Hz", "1000000000000000000000000000000Hz"] {
        let source = Quantity::parse_plot_literal(literal).unwrap();
        assert!(
            prepare_exact_audio_frequency(
                source,
                literal,
                &mut ExactNumericWork::new(1_000_000_000)
            )
            .is_err(),
            "{literal}"
        );
    }
    let source = Quantity::parse_plot_literal("440Hz").unwrap();
    assert!(prepare_exact_audio_frequency(
        source,
        "441Hz",
        &mut ExactNumericWork::new(1_000_000_000)
    )
    .is_err());
    assert!(prepare_exact_audio_frequency(source, "440Hz", &mut ExactNumericWork::new(0)).is_err());
}
