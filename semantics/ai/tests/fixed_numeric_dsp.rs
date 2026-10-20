use conduit_ai::fixed_numeric_dsp::*;
#[test]
fn dft_impulse_dc_tone_and_parseval_match_independent_spectral_identities() {
    let mut output = [0.; 322];
    let mut impulse = [0.; 320];
    impulse[0] = 1.;
    real_dft(&impulse, &mut output).unwrap();
    for bin in output.as_chunks::<2>().0 {
        assert!((bin[0] - 1.).abs() < 1e-6);
        assert!(bin[1].abs() < 1e-6);
    }
    real_dft(&[0.25; 320], &mut output).unwrap();
    assert!((output[0] - 80.).abs() < 1e-5);
    for bin in output[2..].iter() {
        assert!(bin.abs() < 1e-5);
    }
    let tone: [f32; 320] = core::array::from_fn(|n| {
        ((2. * std::f64::consts::PI * 7. * n as f64 / 320.).cos() * 0.5) as f32
    });
    real_dft(&tone, &mut output).unwrap();
    assert!((output[14] - 80.).abs() < 1e-4);
    assert!(output[15].abs() < 1e-4);
    let mut power = [0.; 161];
    magnitude_squared(&output, &mut power).unwrap();
    let spectral = (f64::from(power[0])
        + f64::from(power[160])
        + 2. * power[1..160].iter().map(|&x| f64::from(x)).sum::<f64>())
        / 320.;
    let time = tone
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>();
    assert!((spectral - time).abs() < 1e-4);
}
#[test]
fn numerical_domain_and_overflow_refusals_preserve_caller_output() {
    let mut out = [9.; 322];
    let mut input = [0.; 320];
    input[2] = f32::NAN;
    assert_eq!(real_dft(&input, &mut out), Err(FixedDspRefusal::Nonfinite));
    assert_eq!(out, [9.; 322]);
    input = [f32::MAX; 320];
    assert_eq!(real_dft(&input, &mut out), Err(FixedDspRefusal::Nonfinite));
    assert_eq!(out, [9.; 322]);
    let mut power = [9.; 161];
    assert_eq!(
        magnitude_squared(&[f32::MAX; 322], &mut power),
        Err(FixedDspRefusal::Nonfinite)
    );
    assert_eq!(power, [9.; 161]);
    let mut logs = [9.; 18];
    assert_eq!(
        log10(&[0.; 18], &mut logs),
        Err(FixedDspRefusal::Nonpositive)
    );
    assert_eq!(logs, [9.; 18]);
    assert_eq!(sqrt(-1.), Err(FixedDspRefusal::Negative));
    assert_eq!(sqrt(f32::NAN), Err(FixedDspRefusal::Nonfinite));
    assert_eq!(
        dot(&[f32::MAX; 160], &[2.; 160]),
        Err(FixedDspRefusal::Nonfinite)
    );
}
#[test]
fn logarithm_dot_and_square_root_have_explicit_finite_scalar_meaning() {
    let mut output = [0.; 18];
    log10(&[100.; 18], &mut output).unwrap();
    assert_eq!(output, [2.; 18]);
    assert_eq!(dot(&[0.5; 160], &[0.25; 160]).unwrap(), 20.);
    assert_eq!(sqrt(4.).unwrap(), 2.);
    let mut wrong = [9.; 320];
    assert_eq!(
        real_dft(&[0.; 320], &mut wrong),
        Err(FixedDspRefusal::Shape)
    );
    assert_eq!(wrong, [9.; 320]);
}

#[cfg(feature = "kernel-step")]
#[test]
fn signed_integer_codec_uses_exact_canonical_shape_and_preserves_output_on_refusal() {
    use conduit_ai::{
        fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_i16_codec::FixedI16VectorCodec,
    };
    let mut codec =
        FixedI16VectorCodec::<160>::prepare(&fixed_numeric_type("NumericI16Vector160").unwrap())
            .unwrap();
    let values = core::array::from_fn(|i| match i % 4 {
        0 => i16::MIN,
        1 => i16::MAX,
        2 => -1,
        _ => 0,
    });
    let bytes = codec.encode(&values).to_vec();
    let mut output = [9; 160];
    codec.decode(&bytes, &mut output).unwrap();
    assert_eq!(output, values);
    output = [9; 160];
    let mut malformed = bytes.clone();
    malformed[0] = 255;
    assert!(codec.decode(&malformed, &mut output).is_err());
    assert_eq!(output, [9; 160]);
    assert!(codec
        .decode(&bytes[..bytes.len() - 1], &mut output)
        .is_err());
    assert_eq!(output, [9; 160]);
    assert!(FixedI16VectorCodec::<160>::prepare(
        &fixed_numeric_type("NumericF32Vector160").unwrap()
    )
    .is_err());
}

#[test]
fn recovered_dsp_contracts_use_current_revision_and_value_ports() {
    use conduit_ai::{fixed_numeric_catalog::*, fixed_numeric_dsp_catalog::*};
    for operation in [
        FixedDspOperation::RealDft320,
        FixedDspOperation::MagnitudeSquared161,
        FixedDspOperation::Log10_18,
        FixedDspOperation::Log10_1,
        FixedDspOperation::Dot160,
        FixedDspOperation::Sqrt1,
    ] {
        let kind = fixed_dsp_contract(operation, false).unwrap();
        assert_eq!(
            kind.kind_contract_revision.as_str(),
            format!("{FIXED_NUMERIC_REVISION}/numeric/{}", operation.name())
        );
        for port in kind.inputs.iter().chain(&kind.outputs) {
            assert!(matches!(port.temporal, conduit_core::PortTemporal::Value));
        }
    }
    // Exponential already belongs to the current elementwise owner.
    assert!(fixed_numeric_contract("numeric/exp1").unwrap().is_some());
}

#[test]
fn real_dft320_preserves_forward_phase_shift_and_nyquist() {
    let mut output = [0.; 322];
    let sine = core::array::from_fn::<_, 320, _>(|n| {
        (0.5 * (2. * std::f64::consts::PI * 7. * n as f64 / 320.).sin()) as f32
    });
    real_dft(&sine, &mut output).unwrap();
    assert!(output[14].abs() < 1e-4);
    assert!((output[15] + 80.).abs() < 1e-4); // forward e^(-i angle), not conjugate
    let mut shifted = [0.; 320];
    shifted[1] = 1.;
    real_dft(&shifted, &mut output).unwrap();
    for k in 0..=160 {
        let angle = 2. * std::f64::consts::PI * k as f64 / 320.;
        assert!((f64::from(output[2 * k]) - angle.cos()).abs() < 1e-6);
        assert!((f64::from(output[2 * k + 1]) + angle.sin()).abs() < 1e-6);
    }
    let nyquist = core::array::from_fn::<_, 320, _>(|n| if n % 2 == 0 { 0.25 } else { -0.25 });
    real_dft(&nyquist, &mut output).unwrap();
    assert!((output[320] - 80.).abs() < 1e-5);
    assert!(output[321].abs() < 1e-5);
    assert!(output[..320].iter().all(|v| v.abs() < 1e-5));
    let mut power = [0.; 161];
    magnitude_squared(&output, &mut power).unwrap();
    // Nyquist is counted once, interior bins twice, in a real one-sided spectrum.
    assert!((power[160] / 320. - nyquist.iter().map(|v| v * v).sum::<f32>()).abs() < 1e-5);
}
