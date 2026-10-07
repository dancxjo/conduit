#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_float_integer::*, fixed_numeric_integer_narrowing::*,
    fixed_numeric_temporal::*, fixed_numeric_u16_profile::*,
};
use conduit_core::*;
use conduit_plot::*;
fn catalogs() -> (StartupCatalog, ProfileCatalog, String) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_closing_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_ai::fixed_numeric_dsp_catalog::install_fixed_dsp_flow_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    install_checked_integer_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_float_integer_catalogs(&mut startup, &mut profiles).unwrap();
    PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n")
        .unwrap()
        .install(&mut startup, &mut profiles, true)
        .unwrap();
    (
        startup,
        profiles,
        [
            include_str!("../../speech/fargan_conditioning.conduit"),
            include_str!("../../speech/fargan_epoch_policy.conduit"),
        ]
        .join("\n"),
    )
}
#[test]
fn source_period_rounding_clamps_before_addition_and_preserves_exact_admission() {
    let (startup, profiles, source) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/fargan-period-q8-round", &profiles)
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for value in [0, 8191, 8192, 8319, 8320, 65279, 65280, 65281, u64::MAX] {
        let expected = ((value.clamp(8192, 65280) + 128) / 256).to_le_bytes();
        assert_eq!(program.evaluate(&value.to_le_bytes()).unwrap(), expected);
        assert_eq!(prepared.evaluate(&value.to_le_bytes()).unwrap(), expected);
    }
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-period-from-q8",
        &profiles,
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 3);
    for gear in &expanded.expanded.gears {
        assert!(gear
            .inputs
            .iter()
            .chain(&gear.outputs)
            .all(|p| p.temporal == PortTemporal::Flow { closes: true }));
    }
    let pcm = expand_canonical_plot_for_authoring(&checked, "speech/flow-fargan-pcm16", &profiles)
        .unwrap();
    assert_eq!(pcm.expanded.gears.len(), 3);
}

#[test]
fn source_pcm_scaling_is_explicit_saturation_and_reference_prepared_identical() {
    use conduit_ai::fixed_numeric_codec::FixedF32VectorCodec;
    let (startup, profiles, source) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/flow-fargan-pcm16-scale", &profiles)
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut input = FixedF32VectorCodec::<160>::prepare(&program.input_type).unwrap();
    let output = FixedF32VectorCodec::<160>::prepare_raw(&program.output_type).unwrap();
    let pattern = [
        f32::MIN,
        -2.,
        -1.,
        -0.5 / 32768.,
        0.,
        0.5 / 32768.,
        32767. / 32768.,
        1.,
        2.,
        f32::MAX,
    ];
    let samples = core::array::from_fn(|index| pattern[index % pattern.len()]);
    let bytes = input.encode(&samples).unwrap();
    let expected =
        core::array::from_fn(|index| samples[index].clamp(-1., 32767. / 32768.) * 32768.);
    let reference = program.evaluate(bytes).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(prepared.evaluate(bytes).unwrap(), reference);
    let mut decoded = [0.; 160];
    output.decode(&reference, &mut decoded).unwrap();
    assert_eq!(decoded, expected);
    let pcm =
        conduit_ai::fixed_numeric_float_integer::round_f32_to_i16_160(&decoded, &mut [0; 160]);
    assert!(pcm.is_ok());
}
