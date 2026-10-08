use conduit_audio::{
    AudioCycleDuration, AudioFrequencyHz, AudioResonator, PreparedAcousticReciprocal,
};
use conduit_core::{ConfigurationValue, StructuredInfoValue};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn program(name: &str) -> PortableExpressionProgram {
    let source = format!(
        "{}\n{}",
        include_str!("../types.conduit"),
        include_str!("../acoustic_quantities.conduit")
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn reciprocal_source_executes_exact_authored_fractions() {
    let seam = PreparedAcousticReciprocal::new().unwrap();
    let to_cycle = program("audio/frequency-to-cycle");
    let to_frequency = program("audio/cycle-to-frequency");
    for (numerator, denominator) in [
        (200, 1),
        (400, 2),
        (1, u64::MAX),
        (u64::MAX, 1),
        (u64::MAX, u64::MAX),
    ] {
        let hz = AudioFrequencyHz::new(denominator, numerator).unwrap();
        let input = hz.into_structured().unwrap().canonical_bytes().unwrap();
        let output = to_cycle.evaluate(&input).unwrap();
        let receipt = seam.frequency_to_cycle(&input).unwrap();
        assert_eq!(receipt.input, hz);
        assert_eq!(receipt.original_canonical, input);
        assert_eq!(receipt.result_canonical, output);
        let retained =
            PortableExpressionProgram::from_canonical_hex(receipt.source_program_hex).unwrap();
        assert_eq!(
            retained.evaluate(&receipt.original_canonical).unwrap(),
            receipt.result_canonical
        );
        assert_eq!(
            seam.frequency_to_cycle(&input)
                .unwrap()
                .result
                .encode()
                .unwrap(),
            output
        );
        assert_eq!(
            seam.cycle_to_frequency(&output)
                .unwrap()
                .result
                .encode()
                .unwrap(),
            input
        );
        let cycle = AudioCycleDuration::from_structured(
            StructuredInfoValue::from_canonical_bytes(&output).unwrap(),
        )
        .unwrap();
        assert_eq!(cycle.numerator_seconds(), &denominator);
        assert_eq!(cycle.denominator(), &numerator);
        let roundtrip = to_frequency.evaluate(&output).unwrap();
        assert_eq!(roundtrip, input);
    }
}

#[test]
fn positive_domains_refuse_zero_and_malformed_inputs() {
    for (n, d) in [(0, 1), (1, 0), (0, 0)] {
        assert!(AudioFrequencyHz::new(d, n).is_err());
        assert!(AudioCycleDuration::new(d, n).is_err());
    }
    for name in ["audio/frequency-to-cycle", "audio/cycle-to-frequency"] {
        let p = program(name);
        let seam = PreparedAcousticReciprocal::new().unwrap();
        assert!(p.evaluate(&[0; 16]).is_err());
        assert!(p.evaluate(&[]).is_err());
        // Retain canonical record framing and Type, but forge a zero numerator
        // in the final U64 leaf: the generated owner admission must refuse it.
        let mut canonical = if name == "audio/frequency-to-cycle" {
            AudioFrequencyHz::new(1, 200).unwrap().encode().unwrap()
        } else {
            AudioCycleDuration::new(200, 1).unwrap().encode().unwrap()
        };
        let end = canonical.len();
        canonical[end - 8..].fill(0);
        if name == "audio/frequency-to-cycle" {
            assert!(seam.frequency_to_cycle(&canonical).is_err());
        } else {
            assert!(seam.cycle_to_frequency(&canonical).is_err());
        }
    }
}

#[test]
fn resonator_fields_are_independent_and_not_sample_rate_bounded() {
    let center = AudioFrequencyHz::new(1, 200).unwrap();
    let bandwidth = AudioFrequencyHz::new(3, 100_000).unwrap();
    // Generated record constructors follow source field alphabetical order.
    let resonator = AudioResonator::new(bandwidth, center).unwrap();
    assert_eq!(resonator.center(), &center);
    assert_eq!(resonator.bandwidth(), &bandwidth);
    println!(
        "AudioResonator canonical value={} bytes",
        resonator.encode().unwrap().len()
    );
    assert_eq!(
        AudioResonator::decode(&resonator.encode().unwrap()).unwrap(),
        resonator
    );
}

#[test]
fn native_storage_sizes_are_inspectable() {
    for (name, ty, payload) in [
        (
            "AudioFrequencyHz",
            AudioFrequencyHz::semantic_type().unwrap(),
            16,
        ),
        (
            "AudioCycleDuration",
            AudioCycleDuration::semantic_type().unwrap(),
            16,
        ),
        (
            "AudioResonator",
            AudioResonator::semantic_type().unwrap(),
            32,
        ),
    ] {
        println!(
            "{name}: type={} bytes, fixed payload={payload} bytes",
            ty.canonical_bytes().unwrap().len()
        );
    }
    println!(
        "AudioFrequencyHz canonical value={} bytes",
        AudioFrequencyHz::new(1, 200)
            .unwrap()
            .encode()
            .unwrap()
            .len()
    );
    println!(
        "AudioCycleDuration canonical value={} bytes",
        AudioCycleDuration::new(200, 1)
            .unwrap()
            .encode()
            .unwrap()
            .len()
    );
}
