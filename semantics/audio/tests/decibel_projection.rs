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
fn ratio(n: u64, d: u64, power: bool) -> AudioReferencedLevelRatio {
    AudioReferencedLevelRatio::new(
        basis(power),
        if power {
            AudioLevelRatio::power(d, n)
        } else {
            AudioLevelRatio::amplitude(d, n)
        }
        .unwrap(),
    )
    .unwrap()
}
fn reproduce(executions: &[AudioSourceExecution]) {
    for e in executions {
        assert_eq!(
            conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                .unwrap()
                .evaluate(e.input_canonical())
                .unwrap(),
            e.output_canonical()
        );
    }
}
#[test]
fn all_exact_exponents_and_inverse_execute_source_with_independent_u128_reference() {
    let prepared = PreparedExactPowerOfTenDecibels::new().unwrap();
    for power in [false, true] {
        for exponent in -18i32..=18 {
            let magnitude = 10u128.pow(exponent.unsigned_abs());
            let (n, d) = if exponent < 0 {
                (1, magnitude as u64)
            } else {
                (magnitude as u64, 1)
            };
            let authored = ratio(n, d, power);
            let frame = authored.clone().encode().unwrap();
            let receipt = prepared.ratio_to_decibels(&frame).unwrap();
            assert_eq!(receipt.original(), &authored);
            assert_eq!(receipt.original_canonical(), frame);
            assert_eq!(receipt.result().basis(), authored.basis());
            match receipt.result().value() {
                AudioDecibelValue::Finite(f) => {
                    assert_eq!(*f.denominator(), 1);
                    assert_eq!(
                        *f.numerator_db(),
                        i64::from(exponent) * if power { 10 } else { 20 }
                    );
                }
                _ => panic!("finite"),
            }
            reproduce(receipt.executions());
            let inverse = prepared
                .decibels_to_ratio(receipt.admitted_canonical())
                .unwrap();
            reproduce(inverse.executions());
            let (nn, dd) = match inverse.result().ratio() {
                AudioLevelRatio::Amplitude(x) => (*x.numerator(), *x.denominator()),
                AudioLevelRatio::Power(x) => (*x.numerator(), *x.denominator()),
            };
            assert_eq!(
                u128::from(nn) * u128::from(d),
                u128::from(n) * u128::from(dd)
            );
        }
    }
}
#[test]
fn zero_has_explicit_negative_infinity_and_unreduced_original_is_preserved() {
    let p = PreparedExactPowerOfTenDecibels::new().unwrap();
    for power in [false, true] {
        let r = p
            .ratio_to_decibels(&ratio(0, u64::MAX, power).encode().unwrap())
            .unwrap();
        assert_eq!(r.result().value(), &AudioDecibelValue::NegativeInfinity);
        let inverse = p.decibels_to_ratio(r.admitted_canonical()).unwrap();
        let expected = ratio(0, 1, power);
        assert_eq!(inverse.result(), &expected);
        let original = ratio(u64::MAX, u64::MAX, power);
        let r = p
            .ratio_to_decibels(&original.clone().encode().unwrap())
            .unwrap();
        assert_eq!(r.original(), &original);
        let original = ratio(10_000_000_000_000_000_000, 10, power);
        assert!(p.ratio_to_decibels(&original.encode().unwrap()).is_ok());
    }
}
#[test]
fn unsupported_numeric_profiles_retain_original_and_source_recognition() {
    let p = PreparedExactPowerOfTenDecibels::new().unwrap();
    for original in [
        ratio(2, 1, true),
        ratio(10_000_000_000_000_000_000, 1, true),
        ratio(u64::MAX, u64::MAX - 1, true),
        ratio(101, 10, true),
    ] {
        let frame = original.encode().unwrap();
        match p.ratio_to_decibels(&frame) {
            Err(AudioDecibelRefusal::UnsupportedExactPowerOfTen(r)) => {
                assert_eq!(r.original_canonical(), frame);
                reproduce(r.executions());
            }
            _ => panic!("explicit refusal"),
        }
    }
    for value in [
        AudioDecibelValue::finite(1, 3).unwrap(),
        AudioDecibelValue::finite(2, 20).unwrap(),
        AudioDecibelValue::finite(1, i64::MIN).unwrap(),
        AudioDecibelValue::finite(1, 190).unwrap(),
    ] {
        let frame = AudioDecibelLevel::new(basis(true), value)
            .unwrap()
            .encode()
            .unwrap();
        assert!(matches!(
            p.decibels_to_ratio(&frame),
            Err(AudioDecibelRefusal::UnsupportedExactPowerOfTen(_))
        ));
    }
}
#[test]
fn explicit_same_proportionality_reference_pair_connects_amplitude_power_and_db() {
    let ab = basis(false);
    let pb = basis(true);
    let refs = AudioAmplitudePowerReferences::new(
        ab.reference().clone(),
        pb.reference().clone(),
        AudioAmplitudePowerRelationship::SamePositiveProportionality,
    )
    .unwrap();
    let request = AudioReferencedAmplitudePowerRequest::new(
        AudioRelativeAmplitude::new(1, 10).unwrap(),
        refs,
    )
    .unwrap();
    let square = convert_referenced_amplitude_to_power(&request.clone().encode().unwrap()).unwrap();
    assert_eq!(square.original(), &request);
    assert_eq!(*square.square().result().numerator(), 100);
    let prepared = PreparedExactPowerOfTenDecibels::new().unwrap();
    let a = prepared
        .ratio_to_decibels(
            &AudioReferencedLevelRatio::new(ab, AudioLevelRatio::amplitude(1, 10).unwrap())
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
    let b = prepared
        .ratio_to_decibels(square.admitted_canonical())
        .unwrap();
    assert_eq!(a.result().value(), b.result().value());
}
#[test]
fn source_reference_role_laws_and_public_corrupt_frame_refusal() {
    let b = basis(true);
    assert!(AudioDecibelBasis::new(
        AudioDecibelConvention::AmplitudeTwentyLog10,
        b.reference().clone()
    )
    .is_err());
    assert!(AudioReferencedLevelRatio::new(b, AudioLevelRatio::amplitude(1, 1).unwrap()).is_err());
    let p = PreparedExactPowerOfTenDecibels::new().unwrap();
    let mut frame = ratio(10, 1, true).encode().unwrap();
    frame.pop();
    assert!(matches!(
        p.ratio_to_decibels(&frame),
        Err(AudioDecibelRefusal::Admission(_))
    ));
    assert!(AudioDecibelValue::finite(0, 0).is_err());
}
#[test]
fn actual_native_type_and_value_sizes() {
    let r = ratio(10, 1, true);
    let p = PreparedExactPowerOfTenDecibels::new().unwrap();
    let receipt = p.ratio_to_decibels(&r.clone().encode().unwrap()).unwrap();
    println!(
        "ratio Type={} value={}; dB Type={} value={}",
        AudioReferencedLevelRatio::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        r.encode().unwrap().len(),
        AudioDecibelLevel::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        receipt.admitted_canonical().len()
    );
}
#[test]
fn core_exact_fidelity_report_rejects_invented_preservation_and_changed_target() {
    use conduit_core::projection::*;
    let receipt = PreparedExactPowerOfTenDecibels::new()
        .unwrap()
        .ratio_to_decibels(&ratio(100, 1, true).encode().unwrap())
        .unwrap();
    let domain = AudioExactDecibelProjection::new(&receipt);
    let native = domain.native();
    let policy = AudioExactDecibelPolicy;
    let text = |s| ProjectionText::new(s).unwrap();
    let build = |facts, target| {
        ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("test"),
                    source: text("ratio"),
                    target: Some(text("dB")),
                    projector: text("audio/source"),
                    requested_route: text("exact-power-of-ten"),
                    boundary: None,
                },
                source: receipt.original(),
                target: Some(target),
                facts,
                native: &native,
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
    };
    let facts = domain.facts();
    let report = build(&facts, receipt.result()).unwrap();
    assert!(report.require_exact().is_ok());
    assert!(build(&facts[..1], receipt.result()).is_err());
    let mut forged = domain.facts();
    forged[1] = ProjectionFact::Preserved {
        obligation: text("level-value"),
    };
    assert!(build(&forged, receipt.result()).is_err());
    let changed =
        AudioDecibelLevel::new(basis(true), AudioDecibelValue::finite(1, 30).unwrap()).unwrap();
    assert!(build(&facts, &changed).is_err());
    let mut forged = domain.facts();
    forged[1] = ProjectionFact::Lost {
        obligation: text("level-value"),
        class: ProjectionLoss::Approximation,
        detail: &(),
        native_fact: Some(text("audio-original")),
    };
    assert!(build(&forged, receipt.result()).is_err());
}
