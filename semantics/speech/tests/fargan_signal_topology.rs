use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
const SOURCE: &str = include_str!("../fargan_signal.conduit");
#[test]
fn pinned_signal_and_explicit_gru_gate_topologies_check() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(&format!(
            "{}\n{}\n{}\n{}",
            include_str!("../fargan_conditioning.conduit"),
            SOURCE,
            include_str!("../fargan_pitch_history.conduit"),
            include_str!("../fargan_subframe.conduit")
        )),
        &startup,
    )
    .unwrap();
    for name in [
        "FarganNetworkState",
        "FarganSignalResult",
        "FarganSubframeState",
        "FarganSubframeResult",
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type;
        let bound = conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap();
        assert!(bound <= 16_384);
        println!("{name} canonical bound={bound}B");
    }
    for name in [
        "speech/fargan-gru272x160",
        "speech/fargan-gru240x128",
        "speech/fargan-gru208x128",
        "speech/fargan-signal-subframe",
        "speech/fargan-pitch-conditioning",
        "speech/fargan-history-deemphasis",
        "speech/fargan-pitch-indices",
        "speech/fargan-condition-subframe",
        "speech/fargan-subframe",
    ] {
        conduit_plot::expand_canonical_plot_for_authoring(&checked, name, &profiles)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    }
}

#[test]
fn recurrent_weight_transpose_and_wrong_state_width_refuse_before_execution() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    for (before, after) in [
        (
            ">> recurrent_weights: NumericF32MatrixRef160x480",
            ">> recurrent_weights: NumericF32MatrixRef272x480",
        ),
        (
            ">> prior: NumericF32Vector160",
            ">> prior: NumericF32Vector128",
        ),
    ] {
        let source = format!(
            "{}\n{}",
            include_str!("../fargan_conditioning.conduit"),
            SOURCE.replace(before, after)
        );
        assert_ne!(SOURCE.replace(before, after), SOURCE);
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        assert!(conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "speech/fargan-gru272x160",
            &profiles
        )
        .is_err());
    }
}
#[test]
fn reviewed_gate_order_reset_after_update_and_skip_order_are_explicit() {
    // These assertions retain the pinned-source law; shape checking alone cannot
    // distinguish gates with equal widths or equal-shaped skip contributions.
    for law in [
        "prior >> (0) >> z_input.start",
        "prior >> (160) >> r_input.start",
        "prior >> (320) >> h_input.start",
        "r_sigmoid.result >> reset.left",
        "h_recurrent.result >> reset.right",
        "h_input.result >> candidate_sum.left",
        "reset.result >> candidate_sum.right",
        "z_sigmoid.result >> retained.left",
        "prior >> retained.right",
        "complement.result >> updated.left",
        "candidate.result >> updated.right",
        "glu1.result >> skip1.left",
        "glu2.result >> skip1.right",
        "glu3.result >> skip2.right",
        "conv_glu.result >> skip3.right",
        "prediction4.result >> skip4.right",
        "previous >> skip5.right",
    ] {
        assert!(SOURCE.contains(law), "missing reviewed source law: {law}");
    }
    for n in 1..=3 {
        for law in [
            format!("gru{n}_state >> gru{n}.prior"),
            format!("gru{n}_input_weights >> gru{n}.input_weights"),
            format!("gru{n}_recurrent_weights >> gru{n}.recurrent_weights"),
        ] {
            assert!(
                SOURCE.contains(&law),
                "missing reviewed state/resource route: {law}"
            );
        }
    }
    assert!(!SOURCE.contains("input_bias"));
    assert!(!SOURCE.contains("recurrent_bias"));
}

#[test]
fn authored_pitch_indices_match_pinned_scalar_loop_across_admitted_periods() {
    use conduit_core::{
        ConfigurationValue, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
    };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let source = format!(
        "{}\n{}",
        include_str!("../fargan_conditioning.conduit"),
        include_str!("../fargan_pitch_history.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-pitch-indices",
        &profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("checked indices expression")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let StructuredInfoTypeShape::Nominal { representation, .. } = program.input_type.shape() else {
        panic!("one owned period")
    };
    for period in 32u16..=255 {
        let leaf = StructuredInfoValue::leaf(representation.clone(), period.to_le_bytes().to_vec())
            .unwrap();
        let input = StructuredInfoValue::nominal(program.input_type.clone(), leaf)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let bytes = prepared.evaluate(&input).unwrap();
        let result = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
        let StructuredInfoValueShape::Collection(items) = result.shape() else {
            panic!("44 indices")
        };
        let mut position = 254i32 - i32::from(period);
        for item in items {
            let StructuredInfoValueShape::Leaf(bytes) = item.shape() else {
                panic!("index")
            };
            assert_eq!(
                u16::from_le_bytes(bytes.try_into().unwrap()),
                position.max(0) as u16,
                "period {period}"
            );
            position += 1;
            if position == 256 {
                position -= i32::from(period);
            }
        }
    }
}
