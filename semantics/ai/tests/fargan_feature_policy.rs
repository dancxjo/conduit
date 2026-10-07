#[allow(dead_code)]
#[path = "fargan_signal_graph/plan.rs"]
mod plan;
#[allow(dead_code)]
#[path = "fargan_signal_graph/runtime.rs"]
mod runtime;
#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
#[allow(dead_code)]
#[path = "fargan_signal_graph/state.rs"]
mod state;
use conduit_ai::fixed_numeric_catalog::*;
use conduit_core::*;
use conduit_plot::*;
fn value(ty: &StructuredInfoType, values: &mut impl Iterator<Item = f32>) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), value(representation, values)).unwrap()
        }
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(ty.clone(), values.next().unwrap().to_le_bytes().to_vec())
                .unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length).map(|_| value(element, values)).collect(),
        )
        .unwrap(),
        _ => panic!("finite scalar/vector"),
    }
}
fn floats(v: &StructuredInfoValue) -> Vec<f32> {
    match v.shape() {
        StructuredInfoValueShape::Leaf(v) => {
            vec![f32::from_le_bytes(v.try_into().unwrap())]
        }
        StructuredInfoValueShape::Collection(v) => v.iter().flat_map(floats).collect(),
        _ => panic!("finite vector"),
    }
}
#[test]
fn authored_window_and_sequential_spectral_floor_match_independent_equations() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let mut declarations = String::new();
    for (name, element, width) in [
        ("NumericF32Vector18", "NumericFiniteF32", 18),
        ("NumericRawF32Vector18", "F32", 18),
        ("NumericRawF32Vector320", "F32", 320),
    ] {
        if fixed_numeric_type(name).is_err() {
            declarations += &format!("type {name} = collection {element} = {width}\n");
        }
    }
    let policy = include_str!("../../speech/fargan_feature_policy.conduit")
        .split("# These ordinary resource ports")
        .next()
        .unwrap();
    let source = format!("{declarations}{policy}");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for (plot, inputs) in [
        ("speech/fargan-feature-window", vec![1f32; 320]),
        (
            "speech/fargan-feature-floor",
            (0..18)
                .map(|i| if i % 3 == 0 { 3. } else { -20. + i as f32 })
                .collect(),
        ),
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, plot, &profiles).unwrap();
        let mut encoded = None;
        let mut current = expanded.input_bindings[0].gear_id.clone();
        loop {
            let gear = expanded
                .expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == current)
                .unwrap();
            let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
                panic!("program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
            let input = encoded.get_or_insert_with(|| {
                value(&program.input_type, &mut inputs.iter().copied())
                    .canonical_bytes()
                    .unwrap()
            });
            let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
            let actual = evaluator.evaluate(input).unwrap().to_vec();
            assert_eq!(program.evaluate(input).unwrap(), actual);
            encoded = Some(actual);
            if current == expanded.output_bindings[0].gear_id {
                break;
            }
            current = expanded
                .expanded
                .connections
                .iter()
                .find(|c| c.source_gear_id == current)
                .unwrap()
                .sink_gear_id
                .clone();
        }
        let result = floats(&StructuredInfoValue::from_canonical_bytes(&encoded.unwrap()).unwrap());
        if plot.ends_with("window") {
            for (i, a) in result.iter().enumerate() {
                let j = i.min(319 - i) as f64;
                let expected = (0.5
                    * std::f64::consts::PI
                    * (0.5 * std::f64::consts::PI * (j + 0.5) / 160.)
                        .sin()
                        .powi(2))
                .sin();
                assert!((f64::from(*a) - expected).abs() < 3e-8);
            }
        } else {
            let (mut maximum, mut follow) = (-2f32, -2f32);
            for (a, energy) in result.iter().zip(inputs) {
                let expected = energy.max(maximum - 8.).max(follow - 2.5);
                assert_eq!(*a, expected);
                maximum = maximum.max(expected);
                follow = (follow - 2.5).max(expected);
            }
        }
    }
}
#[test]
fn authored_80_to_160_resampler_preserves_causal_previous_sample_and_normalization() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let policy = include_str!("../../speech/fargan_feature_policy.conduit")
        .split("# These ordinary resource ports")
        .next()
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(policy), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/fargan-formant-linear2x", &profiles)
            .unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("input")
    };
    let samples: Vec<_> = (0..80).map(|i| i as f32 - 40.).collect();
    let input = StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == "previous_raw" {
                        value(f.value_type(), &mut [-41.].into_iter())
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
            .find(|f| f.name() == "next_previous_raw")
            .unwrap()
            .value(),
    );
    assert_eq!(next, [39.]);
    let actual = floats(
        fields
            .iter()
            .find(|f| f.name() == "samples")
            .unwrap()
            .value(),
    );
    let mut previous = -41f64;
    for (i, current) in samples.iter().enumerate() {
        assert_eq!(
            f64::from(actual[2 * i]),
            (previous + f64::from(*current)) / 65536.
        );
        assert_eq!(f64::from(actual[2 * i + 1]), f64::from(*current) / 32768.);
        previous = f64::from(*current);
    }
}

#[test]
fn complete_feature_policy_expands_with_exact_generic_contracts() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let source = format!(
        "type FarganPeriod = U16 in 32..=255\n{}",
        include_str!("../../speech/fargan_feature_policy.conduit")
    ) + include_str!("../../speech/fargan_feature_correlation.conduit");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for name in [
        "speech/fargan-feature-spectrum",
        "speech/fargan-feature-log-pitch",
        "speech/fargan-feature-correlation",
        "speech/fargan-feature20",
        "speech/fargan-feature-wave-append",
        "speech/fargan-feature-lag-indices",
        "speech/fargan-feature-preemphasis",
        "speech/fargan-feature-normalized-correlation",
        "speech/fargan-feature-window-basis",
        "speech/fargan-feature-frame",
    ] {
        let graph = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
        assert!(!graph.expanded.gears.is_empty());
    }
}

#[test]
fn authored_spectral_graph_executes_with_public_analysis_resources() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
    use std::{collections::BTreeMap, f64::consts::PI};
    let source = format!("type FarganPeriod = U16 in 32..=255\n{}", include_str!("../../speech/fargan_feature_policy.conduit"));
    let schema = plan::SourceSchema::for_entry(&source, "speech/fargan-feature-spectrum");
    let load = |bytes: &[u8]| bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect::<Vec<_>>();
    let band = load(include_bytes!("../../../proof/fargan/feature-profile/bands161x18.bin"));
    let bias = load(include_bytes!("../../../proof/fargan/feature-profile/band_bias18.bin"));
    let dct = load(include_bytes!("../../../proof/fargan/feature-profile/dct18x18.bin"));
    let resources: shared::Resources = [
        ("band_weights", vec![161,18], band.clone()),
        ("band_bias", vec![18], bias.clone()),
        ("dct_weights", vec![18,18], dct.clone()),
    ].into_iter().map(|(name, dims, values)| (name.into(), shared::Resource::new(schema.ty(name).clone(), &dims, values))).collect();
    let samples: Vec<f32> = (0..320).map(|n| (0.01*(2.*PI*7.*n as f64/320.).sin()+0.006*(2.*PI*31.*n as f64/320.).sin()) as f32).collect();
    let input = BTreeMap::from([("samples".into(), value(schema.ty("samples"), &mut samples.iter().copied()).canonical_bytes().unwrap())]);
    let start = std::time::Instant::now();
    let selected = plan::prepare_entry_plan(&schema, &source, "speech/fargan-feature-spectrum");
    let planning = start.elapsed();
    let result = runtime::run_entry_plan(selected, &resources, input, runtime::ExecutionMode::Normal).unwrap();
    let actual = floats(&result.value);
    let window: Vec<_> = (0..320).map(|i| {
        let n = i.min(319-i) as f64;
        let w = (PI/2.*(PI/2.*(n+0.5)/160.).sin().powi(2)).sin();
        samples[i] as f64*w
    }).collect();
    let power: Vec<_> = (0..161).map(|k| {
        let re: f64 = window.iter().enumerate().map(|(n,x)| x*(2.*PI*k as f64*n as f64/320.).cos()).sum();
        let im: f64 = window.iter().enumerate().map(|(n,x)| -x*(2.*PI*k as f64*n as f64/320.).sin()).sum();
        re*re+im*im
    }).collect();
    let mut maximum = -2f64;
    let mut follow = -2f64;
    let log_bands: Vec<_> = (0..18).map(|b| {
        let energy = bias[b] as f64+(0..161).map(|k| power[k]*band[k*18+b] as f64).sum::<f64>();
        let value = energy.log10().max(maximum-8.).max(follow-2.5);
        maximum = maximum.max(value);
        follow = (follow-2.5).max(value);
        value
    }).collect();
    let expected: Vec<_> = (0..18).map(|j| (0..18).map(|i| log_bands[i]*dct[i*18+j] as f64).sum::<f64>()-if j==0 {4.} else {0.}).collect();
    let error = actual.iter().zip(&expected).map(|(a,b)| (*a as f64-b).abs()).fold(0f64, f64::max);
    eprintln!("source spectral: nodes={} cords={} planning={planning:?} owner_prepare={:?} execute={:?} max_cepstrum_abs={error}", result.nodes, result.cords, result.preparation, result.execution);
    assert!(error < 0.002, "{error}");
    }).unwrap().join().unwrap();
}

#[test]
fn authored_lag_indices_cover_all_admitted_periods_and_refuse_foreign_scalar() {
    use conduit_ai::fixed_numeric_index_codec::FixedU16IndexCodec;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let source = format!(
        "type FarganPeriod = U16 in 32..=255\n{}",
        include_str!("../../speech/fargan_feature_policy.conduit")
    ) + include_str!("../../speech/fargan_feature_correlation.conduit");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-feature-lag-indices",
        &profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let codec = FixedU16IndexCodec::<160>::prepare(&program.output_type).unwrap();
    fn scalar(ty: &StructuredInfoType, x: u16) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(ty.clone(), scalar(representation, x)).unwrap()
            }
            StructuredInfoTypeShape::Leaf(_) => {
                StructuredInfoValue::leaf(ty.clone(), x.to_le_bytes().to_vec()).unwrap()
            }
            _ => panic!("scalar"),
        }
    }
    for period in 32..=255 {
        let input = scalar(&program.input_type, period)
            .canonical_bytes()
            .unwrap();
        let output = prepared.evaluate(&input).unwrap();
        assert_eq!(program.evaluate(&input).unwrap(), output);
        let mut indices = [0; 160];
        codec.decode(output, &mut indices).unwrap();
        for (i, index) in indices.iter().enumerate() {
            assert_eq!(*index, 480 + i as u16 - period);
        }
    }
    let foreign = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
        80u16.to_le_bytes().to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(prepared.evaluate(&foreign).is_err());
    assert!(program.evaluate(&foreign).is_err());
}

#[path = "fargan_feature_policy/temporal.rs"]
mod temporal;

#[path = "fargan_feature_policy/native.rs"]
mod native;
