use conduit_core::Scalar;

use super::{prepare_logic_multi, run_logic_multi};

#[test]
fn ordinary_compare_and_select_agree_with_shared_portable_semantics() {
    let cases = [
        (
            conduit_semantic_catalog::ScalarComparison::Less,
            Scalar::from_raw_microunits(-2),
            Scalar::from_raw_microunits(3),
        ),
        (
            conduit_semantic_catalog::ScalarComparison::Equal,
            Scalar::from_raw_microunits(5),
            Scalar::from_raw_microunits(4),
        ),
        (
            conduit_semantic_catalog::ScalarComparison::GreaterOrEqual,
            Scalar::from_raw_microunits(9),
            Scalar::from_raw_microunits(9),
        ),
    ];
    for (comparison, left, right) in cases {
        let when_false = Scalar::from_raw_microunits(10);
        let when_true = Scalar::from_raw_microunits(20);
        let prepared = prepare_logic_multi(
            "logic-host",
            "logic-boot",
            left,
            right,
            comparison,
            when_false,
            when_true,
        )
        .unwrap();
        let fragment = &prepared.plan.fragments[0];
        for (kind, implementation) in [
            (
                conduit_semantic_catalog::LOGIC_COMPARE_KIND,
                crate::functional_offers::LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
            ),
            (
                conduit_semantic_catalog::LOGIC_SELECT_KIND,
                crate::functional_offers::LOGIC_SELECT_SCALAR_IMPLEMENTATION,
            ),
        ] {
            assert_eq!(
                fragment
                    .placements
                    .iter()
                    .find(|placement| placement.kind_id.as_str() == kind)
                    .unwrap()
                    .implementation_id
                    .as_str(),
                implementation
            );
        }
        let proof = run_logic_multi(&prepared).unwrap();
        let decision = comparison.evaluate(left, right);
        assert_eq!(proof.decision.get(), decision);
        assert_eq!(
            proof.output,
            conduit_semantic_catalog::select_scalar(decision, when_false, when_true)
        );
    }
}

#[test]
fn plan_identity_seals_comparison_configuration_and_exact_sources() {
    let first = prepare_logic_multi(
        "logic-host",
        "logic-boot",
        Scalar::ZERO,
        Scalar::ONE,
        conduit_semantic_catalog::ScalarComparison::Less,
        Scalar::ZERO,
        Scalar::ONE,
    )
    .unwrap();
    let changed = prepare_logic_multi(
        "logic-host",
        "logic-boot",
        Scalar::ZERO,
        Scalar::ONE,
        conduit_semantic_catalog::ScalarComparison::Equal,
        Scalar::ZERO,
        Scalar::ONE,
    )
    .unwrap();
    assert_ne!(first.plan.plan_id, changed.plan.plan_id);
}
