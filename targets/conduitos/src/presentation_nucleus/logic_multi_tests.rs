use conduit_core::Scalar;

use super::{prepare_logic_multi, run_logic_multi};

#[test]
fn ordinary_compare_and_select_agree_with_shared_portable_semantics() {
    let cases = [
        (
            conduit_semantic_catalog::ScalarComparison::Less,
            Scalar::from_decimal_parts(-2, -6).unwrap(),
            Scalar::from_decimal_parts(3, -6).unwrap(),
        ),
        (
            conduit_semantic_catalog::ScalarComparison::Equal,
            Scalar::from_decimal_parts(5, -6).unwrap(),
            Scalar::from_decimal_parts(4, -6).unwrap(),
        ),
        (
            conduit_semantic_catalog::ScalarComparison::GreaterOrEqual,
            Scalar::from_decimal_parts(9, -6).unwrap(),
            Scalar::from_decimal_parts(9, -6).unwrap(),
        ),
    ];
    for (comparison, left, right) in cases {
        let when_false = Scalar::from_decimal_parts(10, -6).unwrap();
        let when_true = Scalar::from_decimal_parts(20, -6).unwrap();
        let prepared = prepare_logic_multi(
            "logic-host",
            "logic-boot",
            left.clone(),
            right.clone(),
            comparison,
            when_false.clone(),
            when_true.clone(),
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
        let decision = comparison.evaluate(&left, &right).unwrap();
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

#[test]
fn ordinary_logic_transports_non_grid_rationals_without_narrowing() {
    let third = Scalar::integer(1).checked_div(&Scalar::integer(3)).unwrap();
    let rounded = Scalar::parse_decimal("0.33333333333333333333333333333333333333").unwrap();
    let false_value = Scalar::integer(1).checked_div(&Scalar::integer(7)).unwrap();
    let true_value = Scalar::integer(5).checked_div(&Scalar::integer(9)).unwrap();
    let prepared = prepare_logic_multi(
        "logic-rational-host",
        "logic-rational-boot",
        third,
        rounded,
        conduit_semantic_catalog::ScalarComparison::Greater,
        false_value,
        true_value.clone(),
    )
    .unwrap();
    let proof = run_logic_multi(&prepared).unwrap();
    assert_eq!(proof.decision, conduit_core::InfoBool::TRUE);
    assert_eq!(proof.output, true_value);
}

#[test]
fn unknown_symbolic_comparison_emits_declared_refusal_instead_of_false() {
    let prepared = prepare_logic_multi(
        "logic-symbolic-host",
        "logic-symbolic-boot",
        Scalar::pi(),
        Scalar::integer(3),
        conduit_semantic_catalog::ScalarComparison::Less,
        Scalar::ZERO,
        Scalar::ONE,
    )
    .unwrap();
    assert_eq!(
        run_logic_multi(&prepared),
        Err(super::logic_multi_play::LogicMultiError::Numerical(
            conduit_core::NumericRefusal::IndeterminateDomain
        ))
    );
}
