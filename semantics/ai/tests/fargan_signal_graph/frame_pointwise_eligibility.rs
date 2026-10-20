//! Full authored FARGAN Plan semantic eligibility; no execution/speed claim.
use super::*;
use conduit_core::{derive_planned_transformation_eligibility, verify_plan, WorkTransformation};
#[test]
fn full_frame_pointwise_eligibility_is_derived_from_versioned_semantics() {
    super::super::on_large_stack(|| {
        let schema = schema();
        let plan = prepare_entry_plan(&schema, &source(), ENTRY);
        assert!(verify_plan(&plan));
        assert_eq!(plan.fragments.len(), 1);
        let fragment = &plan.fragments[0];
        assert_eq!(fragment.placements.len(), 768);
        assert_eq!(fragment.connections.len(), 1211);
        let pure: Vec<_> = fragment
            .placements
            .iter()
            .filter(|placement| {
                derive_planned_transformation_eligibility(placement)
                    .and_then(|eligibility| eligibility.require(WorkTransformation::Fusion))
                    .is_ok()
            })
            .collect();
        assert_eq!(pure.len(), 191);
        assert_eq!(
            pure.iter()
                .filter(|placement| {
                    placement.kind_id.as_str().ends_with("128")
                        || placement.kind_id.as_str().ends_with("160")
                })
                .count(),
            169
        );
        let mut counts = std::collections::BTreeMap::new();
        for placement in &pure {
            *counts.entry(placement.kind_id.as_str()).or_insert(0usize) += 1;
        }
        eprintln!("full-frame checked pointwise eligibility: {counts:?}");
        for placement in pure {
            assert!(placement
                .kind_contract_revision
                .as_str()
                .ends_with("/pure-value@1"));
            assert!(placement.resources.is_empty());
            assert!(placement.host_calls.is_empty());
            assert!(placement.authority.is_empty());
            let offer =
                conduit_ai::operation_owners::fixed_numeric::fixed_numeric_offer_capacity64(
                    placement.kind_id.as_str(),
                )
                .unwrap();
            conduit_ai::fixed_numeric_preparation::verify_fixed_placement(placement, &offer)
                .unwrap();
        }
        assert!(
            fragment.execution_fusions.is_empty(),
            "eligibility is not selection"
        );
    });
}
