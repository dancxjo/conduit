use conduit_audio::*;
use conduit_core::{ExactNumericWork, Quantity};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn exact_literals_keep_authored_custody_and_transport_admitted_audio_values() {
    for literal in ["440Hz", "0.5kHz", "1.25Hz", "18446744073709551615Hz"] {
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

#[test]
fn authored_phase_retains_exact_scope_and_refuses_foreign_origins() {
    let anchor = |timeline, origin| {
        AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(origin).unwrap(),
            AudioTimelineIdentity::new(timeline).unwrap(),
        )
        .unwrap()
    };
    let time = AudioExactTimeOffset::new(3, 1).unwrap();
    let phase = AudioPhaseReference::new(
        anchor(1, 2),
        time.clone(),
        4,
        -1,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "explicit phase reference".into(),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let decoded = AudioPhaseReference::decode(&phase.clone().encode().unwrap()).unwrap();
    assert_eq!(decoded, phase);
    assert_eq!(*decoded.numerator_turns(), -1);
    assert_eq!(*decoded.denominator(), 4);
    for (timeline, origin, accepted) in [(1, 2, true), (2, 2, false), (1, 3, false)] {
        let query = AudioTrajectoryQuery::new(anchor(timeline, origin), time.clone()).unwrap();
        assert_eq!(
            AudioPhaseReferenceMatch::new(query, phase.clone()).is_ok(),
            accepted
        );
    }
    assert!(AudioPhaseReference::new(
        anchor(1, 2),
        time,
        0,
        1,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "invalid denominator".into(),
            None
        )
        .unwrap()
    )
    .is_err());
}

#[test]
fn checked_source_locals_retain_origin_and_reject_foreign_dimensions_before_transport() {
    use conduit_plot::{
        check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
    };
    for (literal, accepted) in [("440Hz", true), ("440m", false), ("-440Hz", false)] {
        let source = format!("plot controls {{\n frequency = {literal}\n delay = 250ms\n}}\n");
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        assert_eq!(syntax.round_trip(), source);
        assert_eq!(syntax.source_document_id(), checked.source_document_id);
        let CanonicalStartupValue::Quantity(frequency) = &checked.plots[0]
            .local_values
            .iter()
            .find(|(name, _)| name == "frequency")
            .unwrap()
            .1
        else {
            panic!("checked quantity")
        };
        assert_eq!(frequency.source(), literal);
        let prepared = prepare_exact_audio_frequency(
            frequency.value(),
            frequency.source(),
            &mut ExactNumericWork::new(1_000_000_000),
        );
        assert_eq!(prepared.is_ok(), accepted);
        if let Ok(prepared) = prepared {
            assert_eq!(prepared.receipt().original(), frequency.source());
            assert_eq!(*prepared.value().numerator_hz(), 440);
            assert_eq!(*prepared.value().denominator(), 1);
        }
        let CanonicalStartupValue::Quantity(delay) = &checked.plots[0]
            .local_values
            .iter()
            .find(|(name, _)| name == "delay")
            .unwrap()
            .1
        else {
            panic!("checked time")
        };
        let delay = prepare_exact_audio_time(
            delay.value(),
            delay.source(),
            &mut ExactNumericWork::new(1_000_000_000),
        )
        .unwrap();
        assert_eq!(*delay.value().numerator_seconds(), 1);
        assert_eq!(*delay.value().denominator(), 4);
    }
}

#[test]
fn exact_profile_boundaries_refuse_nonfinite_and_preserve_zero_time() {
    for literal in ["NaNHz", "infHz", "-infHz"] {
        assert!(Quantity::parse_plot_literal(literal).is_err(), "{literal}");
    }
    for literal in ["-1s", "1Hz"] {
        assert!(prepare_exact_audio_time(
            Quantity::parse_plot_literal(literal).unwrap(),
            literal,
            &mut ExactNumericWork::new(1_000_000_000)
        )
        .is_err());
    }
    let zero = prepare_exact_audio_time(
        Quantity::parse_plot_literal("0s").unwrap(),
        "0s",
        &mut ExactNumericWork::new(1_000_000_000),
    )
    .unwrap();
    assert_eq!(*zero.value().numerator_seconds(), 0);
    assert_eq!(*zero.value().denominator(), 1);
    assert!(AudioFrequencyHz::new(0, 440).is_err());
    assert!(AudioExactTimeOffset::new(0, 1).is_err());
}
