#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_catalog::*;
const SELECTED_FRAME_BYTES: usize = 16_384;
use conduit_plot::*;
#[test]
fn all_explicit_phase_and_final_pcm16_anchor_records_fit_exact_transport_envelopes() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let source = [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_model_identity.conduit"),
        include_str!("../../speech/fargan_epoch_contracts.conduit"),
        include_str!("../../speech/fargan_epoch_projections.conduit"),
    ]
    .join("\n");
    conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for name in [
        "FarganFloatEpochInput",
        "FarganFloatPhase1",
        "FarganFloatPhase2",
        "FarganFloatPhase3",
        "FarganFloatEpochProposal",
        "FarganPcm16EpochResult",
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let maximum = maximum_prepared_transport_value_bytes(ty).unwrap();
        eprintln!("{name}: max canonical transport {maximum}");
        assert!(
            maximum as usize <= SELECTED_FRAME_BYTES,
            "{name}: {maximum}"
        );
    }
    for phase in 1..=4 {
        for operation in ["carry", "condition", "state", "period"] {
            let name = format!("speech/flow-fargan-epoch-{operation}{phase}");
            let expanded = expand_canonical_plot_for_authoring(&checked, &name, &profiles)
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(expanded.expanded.gears.len(), 1, "{name}");
            assert!(
                expanded.expanded.gears[0]
                    .inputs
                    .iter()
                    .chain(&expanded.expanded.gears[0].outputs)
                    .all(|port| port.temporal == conduit_core::PortTemporal::Flow { closes: true }),
                "{name}"
            );
        }
    }
    let right = &checked
        .native_types
        .iter()
        .find(|t| t.name == "FarganSubframeResult")
        .unwrap()
        .value_type;
    let right_max = maximum_prepared_canonical_value_bytes(right).unwrap();
    {
        let name = "FarganEpochPhaseCarry";
        let left = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let left_max = maximum_prepared_canonical_value_bytes(left).unwrap();
        let pair = conduit_core::PreparedTypedTuplePairEncoder::new(
            left.clone(),
            left_max,
            right.clone(),
            right_max,
        )
        .unwrap();
        eprintln!(
            "{name}: left={left_max}, right={right_max}, paired={}",
            pair.maximum_bytes()
        );
        assert!(pair.maximum_bytes() as usize <= SELECTED_FRAME_BYTES);
    }
    let left = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "FarganEpochFinalCarry")
        .unwrap()
        .value_type;
    let right = fixed_numeric_type("NumericI16Vector160").unwrap();
    let left_max = maximum_prepared_canonical_value_bytes(left).unwrap();
    let right_max = maximum_prepared_canonical_value_bytes(&right).unwrap();
    let pair =
        conduit_core::PreparedTypedTuplePairEncoder::new(left.clone(), left_max, right, right_max)
            .unwrap();
    eprintln!(
        "Final PCM carry: left={left_max}, right={right_max}, paired={}",
        pair.maximum_bytes()
    );
    assert!(pair.maximum_bytes() as usize <= SELECTED_FRAME_BYTES);
}

fn fixture_value(ty: &conduit_core::StructuredInfoType) -> conduit_core::StructuredInfoValue {
    use conduit_core::*;
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), fixture_value(representation)).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name(), fixture_value(field.value_type()))
                        .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length).map(|_| fixture_value(element)).collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Variant { cases, .. } => StructuredInfoValue::variant(
            ty.clone(),
            cases[0].tag(),
            fixture_value(cases[0].payload_type()),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(kind) => StructuredInfoValue::leaf(
            ty.clone(),
            match kind.as_str() {
                "value/f32" | "value/ieee754-binary32" => 0f32.to_le_bytes().to_vec(),
                "value/u16" => 64u16.to_le_bytes().to_vec(),
                "value/u64" => 7u64.to_le_bytes().to_vec(),
                "value/u8" => vec![1],
                "value/unit" => vec![],
                other => panic!("{other}"),
            },
        )
        .unwrap(),
        _ => panic!("unsupported fixture"),
    }
}
fn samples(ty: &conduit_core::StructuredInfoType, start: f32) -> conduit_core::StructuredInfoValue {
    use conduit_core::*;
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), samples(representation, start)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length)
                .map(|index| samples(element, start + f32::from(index)))
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(ty.clone(), start.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("samples"),
    }
}
#[test]
fn authored_phase_carries_drop_consumed_condition_and_preserve_provisional_pcm() {
    use conduit_core::*;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let source = [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_model_identity.conduit"),
        include_str!("../../speech/fargan_epoch_contracts.conduit"),
        include_str!("../../speech/fargan_epoch_projections.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for (phase, remaining, pcm) in [(1, 240, 0), (2, 160, 40), (3, 80, 80), (4, 0, 120)] {
        let expanded = expand_canonical_plot_for_authoring(
            &checked,
            &format!("speech/flow-fargan-epoch-carry{phase}"),
            &profiles,
        )
        .unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
            panic!("input")
        };
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        match field.name() {
                            "condition" => samples(field.value_type(), 0.),
                            "pcm_prefix" => samples(field.value_type(), 1000.),
                            _ => fixture_value(field.value_type()),
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let expected = program.evaluate(&input).unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        let value = StructuredInfoValue::from_canonical_bytes(&expected).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("carry")
        };
        let workspace = fields
            .iter()
            .find(|field| field.name() == "workspace")
            .unwrap()
            .value();
        let codec = conduit_ai::fixed_numeric_codec::FixedF32VectorCodec::<240>::prepare(
            workspace.value_type(),
        )
        .unwrap();
        let mut decoded = [0.; 240];
        codec
            .decode(&workspace.canonical_bytes().unwrap(), &mut decoded)
            .unwrap();
        let expected = core::array::from_fn(|index| {
            if index < remaining {
                80. + index as f32
            } else if index < remaining + pcm {
                1000. + (index - remaining) as f32
            } else {
                0.
            }
        });
        assert_eq!(decoded, expected, "phase{phase}");
    }
}
