use super::*;

#[test]
fn authored_preemphasis_preserves_explicit_prior_input_and_reference_parity() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let policy = format!(
        "type FarganPeriod = U16 in 32..=255\n{}",
        include_str!("../../../speech/fargan_feature_policy.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&policy), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-feature-preemphasis",
        &profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("input")
    };
    let samples: Vec<_> = (0..160).map(|i| (i as f32 - 80.) / 32768.).collect();
    let input = StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == "previous_normalized" {
                        value(f.value_type(), &mut [-81. / 32768.].into_iter())
                    } else {
                        value(f.value_type(), &mut samples.iter().copied())
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let output = prepared.evaluate(&input).unwrap();
    assert_eq!(program.evaluate(&input).unwrap(), output);
    let output = StructuredInfoValue::from_canonical_bytes(output).unwrap();
    let StructuredInfoValueShape::Record(fields) = output.shape() else {
        panic!("result")
    };
    let next = floats(
        fields
            .iter()
            .find(|f| f.name() == "next_previous_normalized")
            .unwrap()
            .value(),
    );
    assert_eq!(next, [79. / 32768.]);
    let actual = floats(
        fields
            .iter()
            .find(|f| f.name() == "samples")
            .unwrap()
            .value(),
    );
    let mut previous = -81f32 / 32768.;
    for (i, current) in samples.iter().enumerate() {
        assert_eq!(actual[i], *current - f32::from_bits(0x3f59999a) * previous);
        previous = *current;
    }
}

#[test]
fn authored_correlation_graph_executes_explicit_period_lag_and_normalization() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use std::collections::BTreeMap;
            let source = format!(
                "type FarganPeriod = U16 in 32..=255\n{}\n{}",
                include_str!("../../../speech/fargan_feature_policy.conduit"),
                include_str!("../../../speech/fargan_feature_correlation.conduit")
            );
            let entry = "speech/fargan-feature-normalized-correlation";
            let schema = plan::SourceSchema::for_entry(&source, entry);
            let waveform: Vec<f32> = (0..640)
                .map(|n| {
                    (0.02 * (n as f64 * 0.083).sin() + 0.005 * (n as f64 * 0.137).cos()) as f32
                })
                .collect();
            let period = 79u16;
            let StructuredInfoTypeShape::Nominal { representation, .. } =
                schema.ty("period").shape()
            else {
                panic!("period profile")
            };
            let period_value = StructuredInfoValue::nominal(
                schema.ty("period").clone(),
                StructuredInfoValue::leaf(representation.clone(), period.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap();
            let input = BTreeMap::from([
                (
                    "waveform".into(),
                    value(schema.ty("waveform"), &mut waveform.iter().copied())
                        .canonical_bytes()
                        .unwrap(),
                ),
                ("period".into(), period_value.canonical_bytes().unwrap()),
            ]);
            let selected = plan::prepare_entry_plan(&schema, &source, entry);
            let result = runtime::run_entry_plan(
                selected,
                &BTreeMap::new(),
                input,
                runtime::ExecutionMode::Normal,
            )
            .unwrap();
            let current = &waveform[480..640];
            let delayed = &waveform[480 - period as usize..640 - period as usize];
            let xx: f64 = current.iter().map(|x| f64::from(*x).powi(2)).sum();
            let yy: f64 = delayed.iter().map(|x| f64::from(*x).powi(2)).sum();
            let xy: f64 = current
                .iter()
                .zip(delayed)
                .map(|(x, y)| f64::from(*x) * f64::from(*y))
                .sum();
            let correlation = (xy / (xx * yy + 1e-15).sqrt()).clamp(-1., 1.);
            let expected = (1. + (5. * correlation).exp()).ln() / (1. + 5f64.exp()).ln() - 0.5;
            let actual = floats(&result.value)[0];
            assert!(
                (f64::from(actual) - expected).abs() < 2e-6,
                "{actual} {expected}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn authored_full_feature_frame_executes_from_one_preemphasized_history() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use std::collections::BTreeMap;
            let source = format!(
                "type FarganPeriod = U16 in 32..=255\n{}\n{}",
                include_str!("../../../speech/fargan_feature_policy.conduit"),
                include_str!("../../../speech/fargan_feature_correlation.conduit")
            );
            let entry = "speech/fargan-feature-frame";
            let schema = plan::SourceSchema::for_entry(&source, entry);
            let load = |bytes: &[u8]| {
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect::<Vec<_>>()
            };
            let band = load(include_bytes!(
                "../../../../proof/fargan/feature-profile/bands161x18.bin"
            ));
            let bias = load(include_bytes!(
                "../../../../proof/fargan/feature-profile/band_bias18.bin"
            ));
            let dct = load(include_bytes!(
                "../../../../proof/fargan/feature-profile/dct18x18.bin"
            ));
            let resources: shared::Resources = [
                ("band_weights", vec![161, 18], band),
                ("band_bias", vec![18], bias),
                ("dct_weights", vec![18, 18], dct.clone()),
            ]
            .into_iter()
            .map(|(name, dims, values)| {
                (
                    name.into(),
                    shared::Resource::new(schema.ty(name).clone(), &dims, values),
                )
            })
            .collect();
            let StructuredInfoTypeShape::Nominal { representation, .. } =
                schema.ty("period").shape()
            else {
                panic!("period")
            };
            let period = StructuredInfoValue::nominal(
                schema.ty("period").clone(),
                StructuredInfoValue::leaf(representation.clone(), 79u16.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap();
            let input = BTreeMap::from([
                (
                    "waveform".into(),
                    value(schema.ty("waveform"), &mut [0f32; 640].into_iter())
                        .canonical_bytes()
                        .unwrap(),
                ),
                ("period".into(), period.canonical_bytes().unwrap()),
            ]);
            let selected = plan::prepare_entry_plan(&schema, &source, entry);
            let result = runtime::run_entry_plan(
                selected,
                &resources,
                input,
                runtime::ExecutionMode::Normal,
            )
            .unwrap();
            let actual = floats(&result.value);
            assert_eq!(actual.len(), 20);
            for j in 0..18 {
                let expected = -2. * (0..18).map(|i| f64::from(dct[i * 18 + j])).sum::<f64>()
                    - if j == 0 { 4. } else { 0. };
                assert!(
                    (f64::from(actual[j]) - expected).abs() < 2e-5,
                    "cepstrum {j}"
                );
            }
            assert!((f64::from(actual[18]) - ((256f64 / 79.).log2() - 1.5)).abs() < 2e-6);
            assert!(
                (f64::from(actual[19]) - (2f64.ln() / (1. + 5f64.exp()).ln() - 0.5)).abs() < 2e-6
            );
            eprintln!(
                "full Source feature20: nodes={} cords={} prepare={:?} execute={:?}",
                result.nodes, result.cords, result.preparation, result.execution
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn exact_authored_period_admission_identity_is_reviewable() {
    let profile = conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n").unwrap();
    eprintln!("exact authored period admission: {}", profile.kind_identity(false));
    assert_eq!(profile.value_type(), &plan::SourceSchema::for_entry(&format!("type FarganPeriod = U16 in 32..=255\n{}",include_str!("../../../speech/fargan_feature_policy.conduit")),"speech/fargan-feature-log-pitch").ty("period").clone());
}
