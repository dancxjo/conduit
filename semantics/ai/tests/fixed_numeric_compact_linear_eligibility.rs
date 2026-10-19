#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_type,
    operation_owners::fixed_numeric::{compact_linear_offer_layout, fixed_numeric_offer},
};
use conduit_core::*;
use conduit_kernel::PortId as KPort;
#[test]
fn reviewed_linear_layouts_keep_exact_distinct_vector_shapes_and_canonical_weights() {
    for (i, o) in [
        (3, 2),
        (161, 18),
        (18, 18),
        (80, 1),
        (328, 192),
        (192, 192),
        (192, 4),
        (272, 480),
        (160, 480),
        (240, 384),
        (128, 384),
        (208, 384),
        (160, 160),
        (128, 128),
        (688, 128),
        (128, 40),
    ] {
        let offer = fixed_numeric_offer(&format!("numeric/linear{i}x{o}")).unwrap();
        let layout = compact_linear_offer_layout(&offer).unwrap();
        assert_eq!(layout.numeric_input.0, KPort(0));
        assert_eq!(layout.output.0, KPort(0));
        assert_eq!(layout.canonical_inputs, [KPort(1)]);
        assert_eq!(layout.numeric_input.1.lanes, i);
        assert_eq!(layout.output.1.lanes, o);
        for (width, physical) in [(i, layout.numeric_input.1), (o, layout.output.1)] {
            let ty = fixed_numeric_type(&format!("NumericF32Vector{width}")).unwrap();
            assert_eq!(
                physical.logical_bytes,
                conduit_plot::maximum_prepared_canonical_value_bytes(&ty).unwrap()
            );
        }
        assert_eq!(offer.inputs[1].port_id, port_id("weights"));
    }
}
#[test]
fn names_partial_metadata_stale_contracts_and_unreviewed_operators_do_not_admit_layouts() {
    let original = fixed_numeric_offer("numeric/linear192x4").unwrap();
    assert!(compact_linear_offer_layout(&original).is_ok());
    for mutation in 0..7 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.implementation.artifact_id = "forged/artifact".into(),
            1 => changed.implementation.execution_profile_id = "forged/profile".into(),
            2 => changed.kind_contract_revision = "stale/contract".into(),
            3 => changed.inputs[0].value_kind = kind_id("forged/vector"),
            4 => changed.inputs[1].port_id = port_id("not-weights"),
            5 => changed
                .semantic_contract
                .laws
                .retain(|l| !matches!(l, KindSemanticLaw::Replay(_))),
            _ => changed.outputs[0].value_kind = kind_id("forged/result"),
        }
        assert!(compact_linear_offer_layout(&changed).is_err());
    }
    for name in ["numeric/add128", "numeric/dense3x2", "numeric/history2x64"] {
        assert!(compact_linear_offer_layout(&fixed_numeric_offer(name).unwrap()).is_err());
    }
}
