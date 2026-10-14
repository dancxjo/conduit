use conduit_audio::*;
use conduit_core::Unit;
use conduit_plot::rust_binding::NativeRustBinding;
fn basis(power: bool) -> AudioDecibelBasis {
    let provenance = AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "declared reference".into(),
        Some("v1".into()),
    )
    .unwrap();
    let reference = AudioDecibelReference::new(
        7,
        "reference-a".into(),
        3,
        provenance,
        if power {
            AudioDecibelReferenceRole::Power
        } else {
            AudioDecibelReferenceRole::AmplitudeMagnitude
        },
        Unit::Second,
    )
    .unwrap();
    AudioDecibelBasis::new(
        if power {
            AudioDecibelConvention::PowerTenLog10
        } else {
            AudioDecibelConvention::AmplitudeTwentyLog10
        },
        reference,
    )
    .unwrap()
}
#[test]
fn positive_exact_profiles_match_independent_decibel_law() {
    let prepared = PreparedPositiveFiniteDecibels::new().unwrap();
    for power in [false, true] {
        for exponent in -18i32..=18 {
            let magnitude = 10u64.pow(exponent.unsigned_abs());
            let (n, d) = if exponent < 0 {
                (1, magnitude)
            } else {
                (magnitude, 1)
            };
            let ratio = if power {
                AudioPositiveLevelRatio::power(d, n).unwrap()
            } else {
                AudioPositiveLevelRatio::amplitude(d, n).unwrap()
            };
            let input = AudioPositiveReferencedLevelRatio::new(basis(power), ratio).unwrap();
            let receipt = prepared
                .ratio_to_decibels(&input.clone().encode().unwrap())
                .unwrap();
            assert_eq!(receipt.original(), &input);
            assert_eq!(
                receipt.original_canonical(),
                input.clone().encode().unwrap()
            );
            let receipt = receipt.conversion();
            assert_eq!(receipt.result().basis(), input.basis());
            let AudioDecibelValue::Finite(value) = receipt.result().value() else {
                panic!("finite")
            };
            assert_eq!(
                *value.numerator_db(),
                i64::from(exponent) * if power { 10 } else { 20 }
            );
            assert_eq!(*value.denominator(), 1);
            let finite = AudioFiniteDecibelLevel::new(
                input.basis().clone(),
                AudioDecibelFraction::new(*value.denominator(), *value.numerator_db()).unwrap(),
            )
            .unwrap();
            let inverse = prepared
                .decibels_to_ratio(&finite.encode().unwrap())
                .unwrap();
            assert_eq!(inverse.conversion().result(), receipt.original());
            for e in receipt
                .executions()
                .iter()
                .chain(inverse.conversion().executions())
            {
                assert_eq!(
                    conduit_plot::PortableExpressionProgram::from_canonical_hex(
                        e.source_program_hex()
                    )
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                    e.output_canonical()
                );
            }
        }
    }
}
#[test]
fn zero_wrong_domain_infinity_and_unsupported_log_profile_refuse() {
    assert!(AudioPositiveRelativeAmplitude::new(1, 0).is_err());
    assert!(AudioPositivePowerRatio::new(1, 0).is_err());
    assert!(AudioPositivePowerRatio::new(0, 1).is_err());
    assert!(AudioPositiveReferencedLevelRatio::new(
        basis(true),
        AudioPositiveLevelRatio::amplitude(1, 1).unwrap()
    )
    .is_err());
    let prepared = PreparedPositiveFiniteDecibels::new().unwrap();
    let infinity =
        AudioDecibelLevel::new(basis(true), AudioDecibelValue::NegativeInfinity).unwrap();
    assert!(prepared
        .decibels_to_ratio(&infinity.encode().unwrap())
        .is_err());
    let unsupported = AudioPositiveReferencedLevelRatio::new(
        basis(true),
        AudioPositiveLevelRatio::power(1, 2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepared.ratio_to_decibels(&unsupported.encode().unwrap()),
        Err(AudioDecibelRefusal::UnsupportedExactPowerOfTen(_))
    ));
}
