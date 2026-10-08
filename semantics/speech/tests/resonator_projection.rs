#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{semantic::*, *};
fn request(f: u64, bw: u64, rate: u64) -> SpeechResonatorProjectionRequest {
    SpeechResonatorProjectionRequest::new(
        SpeechResonatorCoefficientProfile::Q20Series8Q14Nearest,
        conduit_audio::AudioResonator::new(
            conduit_audio::AudioFrequencyHz::new(1, bw).unwrap(),
            conduit_audio::AudioFrequencyHz::new(1, f).unwrap(),
        )
        .unwrap(),
        rate,
    )
    .unwrap()
}
#[test]
fn actual_source_series_and_q14_match_independent_f64_reference() {
    let mut accepted = 0;
    let mut refused = 0;
    for rate in [8000_u64, 16000, 48000] {
        for f in [
            1,
            50,
            270,
            400,
            730,
            870,
            1090,
            1500,
            1700,
            2240,
            2290,
            2440,
            2500,
            2600,
            3010,
            rate * 9 / 20,
        ] {
            if f * 20 > rate * 9 {
                continue;
            }
            for bw in [10, 80, 100, 140, rate / 8] {
                let original = request(f, bw, rate);
                let frame = original.clone().encode().unwrap();
                let receipt = match prepare_speech_resonator_q14(&frame) {
                    Ok(value) => value,
                    Err(SpeechResonatorProjectionRefusal::Quantization(e)) => {
                        let b = i128::from(*e.raw_result().b());
                        let c = i128::from(*e.raw_result().c());
                        assert!(
                            16384 - b - c <= 0
                                || 16384 + b - c <= 0
                                || b * b + 4 * c * 16384 >= 0
                                || c <= -16384
                                || c >= 0
                        );
                        assert_eq!(e.original_canonical(), frame);
                        assert!(!e.executions().is_empty());
                        refused += 1;
                        continue;
                    }
                    Err(e) => panic!(
                        "unexpected numerical-profile failure for f={f},bw={bw},rate={rate}: {e:?}"
                    ),
                };
                accepted += 1;
                assert_eq!(receipt.original(), &original);
                assert_eq!(receipt.original_canonical(), frame);
                let radius = (-std::f64::consts::PI * bw as f64 / rate as f64).exp();
                let b = (2.0
                    * radius
                    * (2.0 * std::f64::consts::PI * f as f64 / rate as f64).cos()
                    * 16384.0)
                    .round() as i64;
                let c = (-radius * radius * 16384.0).round() as i64;
                assert!(
                    (*receipt.result().b() - b).abs() <= 2,
                    "f={f},bw={bw},rate={rate}: b {} vs {b}",
                    receipt.result().b()
                );
                assert!((*receipt.result().c() - c).abs() <= 2);
                let (b, c) = receipt.dsp_coefficients().unwrap();
                assert!(16384 - i64::from(b) - i64::from(c) > 0);
                assert!(16384 + i64::from(b) - i64::from(c) > 0);
                assert!(i64::from(b) * i64::from(b) + 4 * i64::from(c) * 16384 < 0);
                assert_eq!(
                    SpeechStableResonatorQ14::decode(receipt.admitted_canonical()).unwrap(),
                    *receipt.result()
                );
            }
        }
    }
    assert!(accepted > 200);
    assert!(refused > 0);
    println!("accepted={accepted}, quantization-stability-refused={refused}");
}
#[test]
fn unsupported_profiles_and_original_fractions_refuse_without_normalization() {
    for (f, bw, rate) in [
        (4000, 80, 8000),
        (1500, 9, 8000),
        (1500, 1001, 8000),
        (1500, 80, 22050),
        (u64::MAX, 80, 8000),
    ] {
        assert!(prepare_speech_resonator_q14(&request(f, bw, rate).encode().unwrap()).is_err());
    }
    let raw = SpeechResonatorProjectionRequest::new(
        SpeechResonatorCoefficientProfile::Q20Series8Q14Nearest,
        conduit_audio::AudioResonator::new(
            conduit_audio::AudioFrequencyHz::new(1, 80).unwrap(),
            conduit_audio::AudioFrequencyHz::new(2, 3000).unwrap(),
        )
        .unwrap(),
        8000,
    )
    .unwrap();
    assert!(prepare_speech_resonator_q14(&raw.encode().unwrap()).is_err());
    assert!(SpeechStableResonatorQ14::new(32767, -1).is_err());
    assert!(SpeechStableResonatorQ14::new(0, -16384).is_err());
}
#[test]
fn receipts_retain_every_source_raw_and_native_admission() {
    let r = prepare_speech_resonator_q14(&request(1500, 100, 8000).encode().unwrap()).unwrap();
    assert_eq!(r.admitted_polynomial_state_frames().len(), 18);
    assert_eq!(r.executions().len(), 21);
    for e in r.executions() {
        assert_eq!(
            conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                .unwrap()
                .evaluate(e.input_canonical())
                .unwrap(),
            e.output_canonical()
        );
    }
    println!(
        "coefficient request Type {}, stable coefficient Type {}; actual result {}",
        SpeechResonatorProjectionRequest::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        SpeechStableResonatorQ14::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        r.admitted_canonical().len()
    );
}

#[test]
fn projected_units_drive_actual_source_resonator_and_independent_recurrence() {
    for rate in [8000, 16000, 48000] {
        for frequency in [1500, 2500] {
            let mut energies = Vec::new();
            for bandwidth in [80, 140] {
                let receipt = prepare_speech_resonator_q14(
                    &request(frequency, bandwidth, rate).encode().unwrap(),
                )
                .unwrap();
                let (b, c) = receipt.dsp_coefficients().unwrap();
                let effective_radius = (-(c as f64) / 16384.0).sqrt();
                let effective_center = ((b as f64) / (32768.0 * effective_radius)).acos()
                    * rate as f64
                    / (2.0 * std::f64::consts::PI);
                let effective_width = -effective_radius.ln() * rate as f64 / std::f64::consts::PI;
                assert!((effective_center - frequency as f64).abs() < 3.0);
                assert!((effective_width - bandwidth as f64).abs() < 2.0);
                let (mut y1, mut y2) = (0_i32, 0_i32);
                let mut energy = 0_i128;
                for frame in 0..2048 {
                    let drive = if frame == 0 { 16384000 } else { 0 };
                    let expected = (i128::from(drive)
                        + i128::from(b) * i128::from(y1)
                        + i128::from(c) * i128::from(y2))
                        / 16384;
                    let actual = receipt.filter_transition(drive, y1, y2).unwrap();
                    assert_eq!(i128::from(actual), expected);
                    assert!(actual.abs() < 32767);
                    energy += i128::from(actual) * i128::from(actual);
                    y2 = y1;
                    y1 = actual;
                }
                energies.push(energy);
                assert!(receipt
                    .filter_transition(i32::MAX, i32::MAX, i32::MAX)
                    .is_err());
            }
            assert!(
                energies[0] > energies[1],
                "narrower bandwidth must retain more impulse energy"
            );
        }
    }
}

#[test]
fn core_report_requires_explicit_finite_series_precision_policy() {
    use conduit_core::projection::*;
    fn text(s: &str) -> ProjectionText<'_> {
        ProjectionText::new(s).unwrap()
    }
    let receipt =
        prepare_speech_resonator_q14(&request(1500, 80, 16000).encode().unwrap()).unwrap();
    let domain = SpeechResonatorProjection::new(&receipt);
    let facts = domain.facts();
    let native = domain.native();
    let input = ProjectionInput {
        basis: ProjectionBasis {
            report: text("coefficient-test"),
            source: text("original-resonator"),
            target: Some(text("q14")),
            projector: text("speech/checked-source"),
            requested_route: text("explicit-q20-series8"),
            boundary: None,
        },
        source: receipt.original(),
        target: Some(receipt.result()),
        facts: &facts,
        native: &native,
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let policy = SpeechQ20Series8Q14Policy;
    let report = ProjectionReport::new(&domain, &policy, input).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert!(report.require_exact().is_err());
}
