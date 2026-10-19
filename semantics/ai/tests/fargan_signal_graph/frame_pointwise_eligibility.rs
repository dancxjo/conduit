//! Full authored FARGAN Plan semantic eligibility; no execution/speed claim.
use super::*;
use conduit_core::{derive_planned_transformation_eligibility, verify_plan, WorkTransformation};
#[test]
fn full_frame_pointwise_and_readonly_eligibility_is_derived_from_versioned_semantics() {
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
        let pointwise: Vec<_> = pure
            .iter()
            .filter(|p| p.kind_contract_revision.as_str().ends_with("/pure-value@1"))
            .collect();
        let readonly: Vec<_> = pure
            .iter()
            .filter(|p| {
                p.kind_contract_revision
                    .as_str()
                    .ends_with("/read-immutable-input@1")
            })
            .collect();
        assert_eq!(pointwise.len(), 191);
        assert_eq!(readonly.len(), 24);
        assert_eq!(pure.len(), pointwise.len() + readonly.len());
        assert_eq!(
            pointwise
                .iter()
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
        eprintln!("full-frame checked pointwise + read-only eligibility: {counts:?}");
        for placement in pure {
            assert!(placement.resources.is_empty());
            assert!(placement.host_calls.is_empty());
            assert!(placement.authority.is_empty());
            let name = placement.kind_id.as_str();
            let offer = if placement
                .kind_contract_revision
                .as_str()
                .ends_with("/pure-value@1")
            {
                conduit_ai::operation_owners::fixed_numeric::fixed_numeric_offer_capacity64(name)
                    .unwrap()
            } else {
                assert!(placement
                    .kind_contract_revision
                    .as_str()
                    .ends_with("/read-immutable-input@1"));
                assert_eq!(
                    placement.inputs[1].port_id,
                    conduit_core::port_id("weights")
                );
                assert!(placement.semantic_contract.laws.contains(
                    &conduit_core::KindSemanticLaw::ResourceDependence(
                        conduit_core::SemanticDependence::ExplicitInput
                    )
                ));
                conduit_ai::operation_owners::fixed_numeric::fixed_numeric_offer(name).unwrap()
            };
            conduit_ai::fixed_numeric_preparation::verify_fixed_placement(placement, &offer)
                .unwrap();
        }
        assert!(
            fragment.execution_fusions.is_empty(),
            "eligibility is not selection"
        );
    });
}
