//! Source/type proof using owned generic numeric contracts, without synthesis.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

const SOURCE: &str = include_str!("../fargan_conditioning.conduit");

fn fixture_catalog() -> (StartupCatalog, conduit_plot::ProfileCatalog) {
    let mut catalog = StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(&mut catalog, &mut profiles)
        .unwrap();
    let transposed = "type NumericF32MatrixRef64x32 = {\n    resource: ResourceRef\n    content_digest: NumericTensorDigest\n    element: NumericTensorElement\n    columns: U16 in 64..=64\n    rows: U16 in 32..=32\n}\n";
    for ty in check_syntax_document(&parse_syntax_document(transposed), &catalog)
        .unwrap()
        .native_types
    {
        catalog
            .insert_structured_type(ty.name, ty.value_type)
            .unwrap();
    }
    (catalog, profiles)
}

#[test]
fn conditioning_topology_checks_with_explicit_state_and_weights() {
    let (catalog, profiles) = fixture_catalog();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &catalog).unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-conditioning",
        &profiles,
    )
    .expect("exact tensor and state cords expand");
    let network = checked
        .plots
        .iter()
        .find(|p| p.name == "speech/fargan-conditioning")
        .unwrap();
    assert_eq!(network.gears.len(), 12);
    let cords: Vec<Vec<&str>> = network
        .cords
        .iter()
        .map(|cord| {
            cord.stages
                .iter()
                .filter_map(|stage| match stage {
                    conduit_plot::CheckedCordStage::Reference(endpoint) => Some(endpoint.as_str()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    for edge in [
        ["period", "pitch_index.value"],
        ["pitch_index.result", "embedding.index"],
        ["embedding.result", "concatenate.right"],
        ["dense1_bias", "dense1.bias"],
        ["conv_bias", "convolution.bias"],
        ["dense2_bias", "dense2.bias"],
        ["concatenate.result", "dense1.value"],
        ["dense1.result", "activate1.value"],
        ["activate1.result", "window.value"],
        ["history", "window.history"],
        ["convolution.result", "activate2.value"],
        ["activate2.result", "dense2.value"],
        ["dense2.result", "activate3.value"],
        ["activate3.result", "condition"],
        ["previous_period", "delayed_pitch.value"],
        ["period", "retained_pitch.value"],
    ] {
        assert!(
            cords.iter().any(|cord| cord.as_slice() == edge),
            "missing checked edge {edge:?}"
        );
    }

    assert_eq!(network.runtime_front.inputs().len(), 11);
    assert_eq!(network.runtime_front.outputs().len(), 4);
}

#[test]
fn transposed_weight_shape_is_rejected_at_the_authored_port() {
    // The transposed tensor profile is installed only by the test catalog.
    let source = SOURCE.replace(
        ">> dense1_weights: NumericF32MatrixRef32x64",
        ">> dense1_weights: NumericF32MatrixRef64x32",
    );
    assert_shape_refusal(&source);
}

#[test]
fn wrong_feature_dimension_is_rejected_before_execution() {
    let source = SOURCE.replace(
        ">> features: NumericF32Vector20",
        ">> features: NumericF32Vector32",
    );
    assert_shape_refusal(&source);
}

#[test]
fn wrong_history_shape_is_rejected_before_execution() {
    let source = SOURCE.replace(
        ">> history: NumericHistory2x64",
        ">> history: NumericF32Vector128",
    );
    assert_shape_refusal(&source);
}

fn assert_shape_refusal(source: &str) {
    let (catalog, profiles) = fixture_catalog();
    let checked = check_syntax_document(&parse_syntax_document(source), &catalog)
        .expect("well-formed shape fixture parses");
    let refusal = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-conditioning",
        &profiles,
    )
    .expect_err("incompatible exact shape refuses expansion");
    assert_eq!(refusal.code, "CND-FRM-045");
    assert!(refusal.message.contains("runtime front port"));
}

#[test]
fn wrong_bias_profile_is_rejected_before_execution() {
    assert_shape_refusal(&SOURCE.replace(
        ">> dense1_bias: NumericF32BiasRef64",
        ">> dense1_bias: NumericF32BiasRef128",
    ));
}
