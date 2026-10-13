use super::math_clamp_play::{prepare_clamp, run_clamp};
use conduit_core::{ArtifactId, Scalar};

#[test]
fn ordinary_clamp_plot_handles_below_inside_and_above_boundaries() {
    for (input, expected) in [
        (
            Scalar::from_decimal_parts(-2_000_000 as i128, -6).unwrap(),
            Scalar::from_decimal_parts(-1_000_000 as i128, -6).unwrap(),
        ),
        (
            Scalar::from_decimal_parts(-1_000_000 as i128, -6).unwrap(),
            Scalar::from_decimal_parts(-1_000_000 as i128, -6).unwrap(),
        ),
        (Scalar::ZERO, Scalar::ZERO),
        (
            Scalar::from_decimal_parts(1_000_000 as i128, -6).unwrap(),
            Scalar::from_decimal_parts(1_000_000 as i128, -6).unwrap(),
        ),
        (
            Scalar::from_decimal_parts(2_000_000 as i128, -6).unwrap(),
            Scalar::from_decimal_parts(1_000_000 as i128, -6).unwrap(),
        ),
    ] {
        let prepared = prepare_clamp("clamp-host", "clamp-boot", input.clone()).unwrap();
        let placement = prepared.plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::MATH_CLAMP_KIND)
            .unwrap();
        assert_eq!(
            placement.implementation_id.as_str(),
            crate::functional_offers::MATH_CLAMP_IMPLEMENTATION
        );
        assert_eq!(run_clamp(&prepared).unwrap().output, expected);
    }
}

#[test]
fn invalid_configuration_and_mutated_plan_identity_are_refused() {
    let mut prepared = prepare_clamp("clamp-host", "clamp-boot", Scalar::ZERO).unwrap();
    let transform = prepared.plan.fragments[0]
        .placements
        .iter_mut()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::MATH_CLAMP_KIND)
        .unwrap();
    transform.artifact_id = ArtifactId::from("mutated/clamp");
    assert!(!conduit_core::verify_plan(&prepared.plan));
    assert_eq!(
        conduit_semantic_catalog::clamp_scalar(
            Scalar::ZERO,
            Scalar::from_decimal_parts(1 as i128, -6).unwrap(),
            Scalar::from_decimal_parts(-1 as i128, -6).unwrap(),
        ),
        Err(conduit_semantic_catalog::MathScalarRefusal::InvalidConfiguration)
    );
}
