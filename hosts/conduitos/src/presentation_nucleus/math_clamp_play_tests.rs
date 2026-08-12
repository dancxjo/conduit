use super::math_clamp_play::{prepare_clamp, run_clamp};
use conduit_core::{ArtifactId, Scalar};

#[test]
fn ordinary_clamp_form_handles_below_inside_and_above_boundaries() {
    for (input, expected) in [
        (
            Scalar::from_raw_microunits(-2_000_000),
            Scalar::from_raw_microunits(-1_000_000),
        ),
        (
            Scalar::from_raw_microunits(-1_000_000),
            Scalar::from_raw_microunits(-1_000_000),
        ),
        (Scalar::ZERO, Scalar::ZERO),
        (
            Scalar::from_raw_microunits(1_000_000),
            Scalar::from_raw_microunits(1_000_000),
        ),
        (
            Scalar::from_raw_microunits(2_000_000),
            Scalar::from_raw_microunits(1_000_000),
        ),
    ] {
        let prepared = prepare_clamp("clamp-host", "clamp-boot", input).unwrap();
        let placement = prepared.plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == conduit_std_catalog::MATH_CLAMP_KIND)
            .unwrap();
        assert_eq!(
            placement.implementation_id.as_str(),
            conduit_std_catalog::CONDUITOS_MATH_CLAMP_IMPLEMENTATION
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
        .find(|p| p.kind_id.as_str() == conduit_std_catalog::MATH_CLAMP_KIND)
        .unwrap();
    transform.artifact_id = ArtifactId::from("mutated/clamp");
    assert!(!conduit_core::verify_plan(&prepared.plan));
    assert_eq!(
        conduit_std_catalog::clamp_scalar(
            Scalar::ZERO,
            Scalar::from_raw_microunits(1),
            Scalar::from_raw_microunits(-1),
        ),
        Err(conduit_std_catalog::MathScalarError::InvalidConfiguration)
    );
}
