//! Read-only projection keeps the logical graph beside its physical choice.
use crate::*;
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;

#[test]
fn fusion_inspection_retains_exact_logical_placements_and_cords() {
    let plot = conduit_plot::parse(
        "plot fusion (\n input: Scalar >> output: Scalar\n) {\n a: math/clamp\n b: math/scale\n input >> a >> b >> output\n}\n",
        &conduit_semantic_catalog::standard_profile_catalog(),
    ).unwrap();
    let mut advertisement = conduit_signal_conformance::pico_local_advertisement();
    advertisement.resources.clear();
    advertisement.capabilities = [
        conduit_semantic_catalog::math_clamp_semantic_contract(),
        conduit_semantic_catalog::math_scale_semantic_contract(),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| {
        BackOfferBuilder::new(
            kind,
            Back {
                capability_id: alloc::format!("fixture/fusion/{index}").into(),
                execution_profile_id: "fixture/local".into(),
                implementation_id: "fixture/pure".into(),
                artifact_id: "fixture/artifact".into(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build()
    })
    .collect();
    let choices =
        conduit_planner::default_placements(&plot, core::slice::from_ref(&advertisement)).unwrap();
    let plan = conduit_planner::plan_with_options(
        &plot,
        core::slice::from_ref(&advertisement),
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let mut fragments = plan.fragments.clone();
    let mut placements = fragments[0]
        .placements
        .iter()
        .map(|p| p.placement_id.clone())
        .collect::<alloc::vec::Vec<_>>();
    placements.sort();
    let mut cords = fragments[0]
        .connections
        .iter()
        .map(|c| c.connection_id.clone())
        .collect::<alloc::vec::Vec<_>>();
    cords.sort();
    let fusion = PlannedFusion {
        fusion_id: "fixture/fusion".into(),
        execution_profile_id: "fixture/physical".into(),
        implementation_id: "fixture/fused".into(),
        artifact_id: "fixture/fused-artifact".into(),
        preserved_placements: placements,
        preserved_connections: cords,
    };
    fragments[0].execution_fusions.push(fusion.clone());
    let optimized = seal_plan_with_realization_backs_and_completion(
        plot.identity(),
        plan.completion_policy,
        plan.realization_backs.clone(),
        fragments,
    );
    assert!(verify_plan(&optimized));
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|c| CapabilityStatusReport {
                    capability_id: c.capability_id.clone(),
                    freshness: OfferFreshness::Fresh,
                    support: CapabilitySupport::Supported,
                    availability: CapabilityAvailability::Available,
                })
                .collect(),
            advertisement,
            devices: vec![],
            state: OperationalState::Available,
        }],
        bases: vec![],
        lines: vec![],
        plans: vec![optimized],
        plays: vec![],
        observations: vec![],
        historical_observations: vec![],
        sealed_boot_provenance: vec![],
        retention: RetentionReport {
            item_capacity: 1,
            retained_items: 0,
            dropped_items: 0,
        },
    };
    let report = build_report(&snapshot).unwrap();
    assert_eq!(report.placements.len(), 2);
    assert_eq!(report.connections.len(), 1);
    assert_eq!(report.execution_fusions.len(), 1);
    assert_eq!(report.execution_fusions[0].fusion, fusion);
    let text = render_text_report(&report);
    assert!(text.contains("execution_fusion"));
    for placement in &fusion.preserved_placements {
        assert!(text.contains(placement.as_str()));
    }
    for cord in &fusion.preserved_connections {
        assert!(text.contains(cord.as_str()));
    }
    let retained: ObservatoryReport =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(retained, report);
}
