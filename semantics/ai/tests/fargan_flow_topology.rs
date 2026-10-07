#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_embedding_flow::install_embedding_flow_catalogs,
    fixed_numeric_flow::install_affine_flow_catalogs,
    fixed_numeric_linear_flow::install_linear_flow_catalogs, fixed_numeric_pair_catalog::*,
    fixed_numeric_pair_flow::install_fixed_flow_pair_catalogs,
    fixed_numeric_temporal::install_closing_numeric_catalogs,
};
use conduit_core::*;
use conduit_plot::*;
fn source() -> String {
    [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_conditioning_flow.conduit"),
        include_str!("../../speech/fargan_signal_flow.conduit"),
        include_str!("../../speech/fargan_pitch_history_flow.conduit"),
        include_str!("../../speech/fargan_subframe_flow.conduit"),
    ]
    .join("\n")
}
#[test]
fn explicit_closing_flow_topology_preserves_value_tensor_ports_and_exact_bounds() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_fixed_numeric_pair_catalogs(&mut startup, &mut profiles).unwrap();
    install_affine_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_linear_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_embedding_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_closing_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_fixed_flow_pair_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source()), &startup).unwrap();
    for name in [
        "speech/flow-fargan-conditioning",
        "speech/flow-fargan-signal-subframe",
        "speech/flow-fargan-subframe",
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
        assert!(!expanded.expanded.gears.is_empty());
        for gear in &expanded.expanded.gears {
            for port in gear.inputs.iter().chain(&gear.outputs) {
                let tensor = ["weights", "bias"].contains(&port.port_id.as_str());
                assert_eq!(
                    port.temporal,
                    if tensor {
                        PortTemporal::Value
                    } else {
                        PortTemporal::Flow { closes: true }
                    },
                    "{}:{}",
                    gear.kind_id.as_str(),
                    port.port_id.as_str()
                );
            }
            for law in &gear.semantic_contract.laws {
                if let KindSemanticLaw::ValueContracts(values) = law {
                    for value in values {
                        assert!(
                            value.contract.maximum_bytes as usize
                                <= MAXIMUM_STRUCTURED_CANONICAL_BYTES
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn closing_conditioning_rejects_transposed_or_foreign_tensor_shapes() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_fixed_numeric_pair_catalogs(&mut startup, &mut profiles).unwrap();
    install_affine_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_linear_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_embedding_flow_catalogs(&mut startup, &mut profiles).unwrap();
    install_closing_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_fixed_flow_pair_catalogs(&mut startup, &mut profiles).unwrap();
    for shape in ["NumericF32MatrixRef64x32", "NumericF32MatrixRef192x128"] {
        let authored = source().replace(
            "dense1_weights: NumericF32MatrixRef32x64",
            &format!("dense1_weights: {shape}"),
        );
        let refused = match check_syntax_document(&parse_syntax_document(&authored), &startup) {
            Err(_) => true,
            Ok(checked) => expand_canonical_plot_for_authoring(
                &checked,
                "speech/flow-fargan-conditioning",
                &profiles,
            )
            .is_err(),
        };
        assert!(refused, "{shape}");
    }
}
