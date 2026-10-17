use conduit_core::Scalar;

use super::{prepare_flow_state, run_flow_state};

#[test]
fn ordinary_latest_and_tee_plan_and_run_through_the_fixed_kernel() {
    let value = Scalar::from_raw_microunits(42);
    let prepared = prepare_flow_state("flow-host", "flow-boot", value).unwrap();
    let fragment = &prepared.plan.fragments[0];
    for (kind, implementation) in [
        (
            conduit_semantic_catalog::LATEST_KIND,
            crate::functional_offers::STATE_LATEST_SCALAR_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::TEE_KIND,
            crate::functional_offers::FLOW_TEE_SCALAR_IMPLEMENTATION,
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
    let proof = run_flow_state(&prepared).unwrap();
    assert_eq!(proof.left, value);
    assert_eq!(proof.right, value);
}

#[test]
fn plan_identity_seals_the_exact_scalar_source() {
    let first = prepare_flow_state("flow-host", "flow-boot", Scalar::ZERO).unwrap();
    let changed = prepare_flow_state("flow-host", "flow-boot", Scalar::ONE).unwrap();
    assert_ne!(first.plan.plan_id, changed.plan.plan_id);
}
