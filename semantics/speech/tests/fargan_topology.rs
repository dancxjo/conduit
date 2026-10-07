//! Source/type proof only. Fixture Kind Fores do not offer executable Backs.
use conduit_core::{port_id, CheckedFront, PortDescriptor, PortDirection, PortTemporal};
use conduit_plot::{check_syntax_document, parse_syntax_document, KindSignature, StartupCatalog};

const SOURCE: &str = include_str!("../fargan_conditioning.conduit");

fn fixture_catalog() -> (StartupCatalog, conduit_plot::ProfileCatalog) {
    let declarations = format!("{}\ntype FarganMatrix64x32 = {{\n    resource: ResourceRef\n    content_digest: FarganTensorDigest\n    element: FarganTensorElement\n    columns: U16 in 64..=64\n    rows: U16 in 32..=32\n}}\n", SOURCE.split("plot speech/").next().unwrap());
    let mut base = StartupCatalog::new();
    base.insert_value_kind_alias(
        "ResourceRef",
        conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )
    .unwrap();
    let types = check_syntax_document(&parse_syntax_document(&declarations), &base)
        .expect("finite numeric declarations check");
    let mut catalog = base;
    let mut profiles = conduit_plot::ProfileCatalog::new();
    for ty in types
        .native_types
        .iter()
        .filter(|ty| ty.name == "FarganMatrix64x32")
    {
        catalog
            .insert_structured_type(&ty.name, ty.value_type.clone())
            .unwrap();
    }
    let port = |name: &str, ty: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: if ty == "U16" {
            conduit_core::kind_id("value/u16")
        } else {
            types
                .native_types
                .iter()
                .find(|t| t.name == ty)
                .unwrap()
                .value_type
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        },
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    // Each fixture is a generic numeric contract; ordering lives only in Source.
    for (kind, inputs, outputs) in [
        (
            "numeric/embedding224x12",
            vec![("index", "U16"), ("weights", "FarganEmbedding224x12")],
            vec![("result", "FarganVector12")],
        ),
        (
            "numeric/concatenate20x12",
            vec![("left", "FarganVector20"), ("right", "FarganVector12")],
            vec![("result", "FarganVector32")],
        ),
        (
            "numeric/dense32x64",
            vec![
                ("value", "FarganVector32"),
                ("weights", "FarganMatrix32x64"),
                ("bias", "FarganBias64"),
            ],
            vec![("result", "FarganVector64")],
        ),
        (
            "numeric/dense192x128",
            vec![
                ("value", "FarganVector192"),
                ("weights", "FarganMatrix192x128"),
                ("bias", "FarganBias128"),
            ],
            vec![("result", "FarganVector128")],
        ),
        (
            "numeric/dense128x320",
            vec![
                ("value", "FarganVector128"),
                ("weights", "FarganMatrix128x320"),
                ("bias", "FarganBias320"),
            ],
            vec![("result", "FarganVector320")],
        ),
        (
            "numeric/tanh64",
            vec![("value", "FarganVector64")],
            vec![("result", "FarganVector64")],
        ),
        (
            "numeric/tanh128",
            vec![("value", "FarganVector128")],
            vec![("result", "FarganVector128")],
        ),
        (
            "numeric/tanh320",
            vec![("value", "FarganVector320")],
            vec![("result", "FarganVector320")],
        ),
        (
            "numeric/history2x64",
            vec![
                ("value", "FarganVector64"),
                ("history", "FarganConditionHistory"),
            ],
            vec![
                ("window", "FarganVector192"),
                ("next_history", "FarganConditionHistory"),
            ],
        ),
    ] {
        profiles
            .insert(conduit_plot::KindProjection {
                kind_id: conduit_core::kind_id(kind),
                kind_contract_revision: conduit_core::KindIdentity::from(format!(
                    "{kind}@checker-fixture"
                )),
                inputs: inputs
                    .iter()
                    .map(|(n, t)| port(n, t, PortDirection::Input))
                    .collect(),
                outputs: outputs
                    .iter()
                    .map(|(n, t)| port(n, t, PortDirection::Output))
                    .collect(),
                configuration: vec![],
            })
            .unwrap();
        catalog
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        catalog
            .insert_fore(
                kind,
                CheckedFront::new(
                    vec![],
                    inputs
                        .into_iter()
                        .map(|(n, t)| port(n, t, PortDirection::Input))
                        .collect(),
                    outputs
                        .into_iter()
                        .map(|(n, t)| port(n, t, PortDirection::Output))
                        .collect(),
                    None,
                ),
            )
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
        ["window.window", "convolution.value"],
        ["convolution.result", "activate2.value"],
        ["activate2.result", "dense2.value"],
        ["dense2.result", "activate3.value"],
        ["activate3.result", "condition"],
        ["window.next_history", "next_history"],
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
        ">> dense1_weights: FarganMatrix32x64",
        ">> dense1_weights: FarganMatrix64x32",
    );
    assert_shape_refusal(&source);
}

#[test]
fn wrong_feature_dimension_is_rejected_before_execution() {
    let source = SOURCE.replace(">> features: FarganVector20", ">> features: FarganVector32");
    assert_shape_refusal(&source);
}

#[test]
fn wrong_history_shape_is_rejected_before_execution() {
    let source = SOURCE.replace(
        ">> history: FarganConditionHistory",
        ">> history: FarganVector128",
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
        ">> dense1_bias: FarganBias64",
        ">> dense1_bias: FarganBias128",
    ));
}
