use std::collections::BTreeMap;

use conduit_core::{GearId, OfferGeneration};
use conduit_planner::{
    plan_selected_optimization, select_fusion_candidate, CandidatePlacementDisposition,
    FusionBoundary, FusionPlanningInputs, PlanningOptions,
};

mod common;

#[path = "common/fusion_fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn safe_local_fusion_beats_unfused_and_tiny_remote_compute_gain() {
    let fixture = fixture();
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[offer(&fixture)],
            observations: &[fusion_observation(120)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("three exact candidates compare");
    assert_eq!(
        selection.selected_candidate_id, "all-local-fused",
        "{:?}",
        selection.considered
    );
    assert_eq!(selection.considered[0].total_work_units, 230);
    assert_eq!(selection.considered[1].total_work_units, 120);
    assert_eq!(selection.considered[2].transported_bytes, 100_000);
    assert!(selection.considered[2].total_work_units > 210);
    assert!(selection.explain().contains("safely fused"));

    let optimized = plan_selected_optimization(
        &fixture.plot,
        &fixture.hosts,
        &selection,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("fusion explanation wraps the ordinary plan");
    assert!(optimized.verify());
    assert_eq!(optimized.plan.checked_plot_id, fixture.plot.checked_plot_id);
    assert_eq!(optimized.plan.fragments[0].placements.len(), 3);
    assert_eq!(optimized.plan.fragments[0].connections.len(), 2);
    assert_eq!(optimized.plan.fragments[0].execution_fusions.len(), 1);
    let planned_fusion = &optimized.plan.fragments[0].execution_fusions[0];
    assert!(optimized.plan.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.implementation_id != planned_fusion.implementation_id));
    let lowered =
        conduit_plan_lowering::lowering::lower_plan_fragment(&optimized.plan.fragments[0])
            .expect("selected fusion lowers through the ordinary numeric graph");
    assert_eq!(lowered.nodes.len(), 3);
    assert_eq!(lowered.cords.len(), 2);
    assert_eq!(lowered.fusions.len(), 1);
    assert_eq!(lowered.fusions[0].nodes.len(), 3);
    assert_eq!(lowered.fusions[0].cords.len(), 2);

    let unfused_selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[offer(&fixture)],
            observations: &[fusion_observation(120)],
            boundaries: &[FusionBoundary {
                source_gear_id: GearId::from("fusion/transform"),
                sink_gear_id: GearId::from("fusion/analysis"),
                requires_observation: true,
                requires_authority: false,
            }],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("required observation retains the ordinary local realization");
    let unfused = plan_selected_optimization(
        &fixture.plot,
        &fixture.hosts,
        &unfused_selection,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("unfused ordinary plan");
    assert_ne!(optimized.plan.plan_id, unfused.plan.plan_id);
    assert_eq!(
        optimized.plan.fragments[0].cancellation_policy,
        unfused.plan.fragments[0].cancellation_policy
    );
    assert_eq!(
        optimized.plan.fragments[0].connections,
        unfused.plan.fragments[0].connections
    );

    let mut mutated = optimized.plan.clone();
    mutated.fragments[0].execution_fusions[0]
        .preserved_connections
        .clear();
    assert!(!conduit_core::verify_plan(&mutated));
}

#[test]
fn remote_split_wins_only_when_advantage_exceeds_line_cost() {
    let fixture = fixture();
    let expensive_fusion = fusion_observation(3_000);
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 0),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[offer(&fixture)],
            observations: &[expensive_fusion],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("substantial remote advantage wins despite finite setup cost");
    assert_eq!(selection.selected_candidate_id, "remote-split");
    assert_eq!(selection.considered[2].compute_work_units, 110);
    assert_eq!(selection.considered[2].transport_work_units, 45);
}

#[test]
fn observation_authority_and_semantic_preservation_can_forbid_fusion() {
    let fixture = fixture();
    for boundary in [
        FusionBoundary {
            source_gear_id: GearId::from("fusion/transform"),
            sink_gear_id: GearId::from("fusion/analysis"),
            requires_observation: true,
            requires_authority: false,
        },
        FusionBoundary {
            source_gear_id: GearId::from("fusion/transform"),
            sink_gear_id: GearId::from("fusion/analysis"),
            requires_observation: false,
            requires_authority: true,
        },
    ] {
        let selection = select_fusion_candidate(
            &fixture.plot,
            &fixture.hosts,
            &candidates(&fixture),
            &basis(&fixture, 2),
            FusionPlanningInputs {
                semantic_kinds: &fixture.semantic_kinds,
                offers: &[offer(&fixture)],
                observations: &[fusion_observation(120)],
                boundaries: &[boundary],
                line_offers: std::slice::from_ref(&fixture.line),
            },
        )
        .expect("unfused candidate remains available");
        assert_eq!(selection.selected_candidate_id, "all-local-unfused");
        assert!(matches!(
            selection.considered[1].disposition,
            CandidatePlacementDisposition::Rejected(_)
        ));
    }

    let mut unsafe_offer = offer(&fixture);
    unsafe_offer.preserves_cancellation = false;
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[unsafe_offer],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive an unsafe fusion offer");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");

    let mut effectful_offer = offer(&fixture);
    effectful_offer
        .host_calls
        .push(conduit_core::HostCallRequirement {
            contract_id: conduit_core::HostCallContractId::from("test/ambient-effect"),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 1,
        });
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[effectful_offer],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive an effectful fusion Back");
    let CandidatePlacementDisposition::Rejected(reason) = &selection.considered[1].disposition
    else {
        panic!("fusion must refuse an effectful replacement Back")
    };
    assert!(reason.contains("BackHostCall"));

    let mut stale_offer = offer(&fixture);
    stale_offer.offer_generation = OfferGeneration(2);
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: &fixture.semantic_kinds,
            offers: &[stale_offer],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive a stale Host fusion offer");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");
    assert!(matches!(
        selection.considered[1].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));

    let incomplete_semantics = &fixture.semantic_kinds[..2];
    let selection = select_fusion_candidate(
        &fixture.plot,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            semantic_kinds: incomplete_semantics,
            offers: &[offer(&fixture)],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive missing transformation facts");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");
    let CandidatePlacementDisposition::Rejected(reason) = &selection.considered[1].disposition
    else {
        panic!("fusion must refuse without every exact semantic contract")
    };
    assert!(reason.contains("lacks exact semantic transformation facts"));
}
