//! Checked-source planner eligibility, not fused runtime execution.
use conduit_core::*;
use conduit_planner::*;
use std::collections::BTreeMap;
mod common;
#[path = "common/fusion_fixture.rs"]
mod fixture;

fn plan_chain(count: usize, uniform: bool) -> Result<OptimizedPlan, PlannerError> {
    let mut f = fixture::fixture();
    let mut observations = fixture::basis(&f, 2);
    let mut source = "plot fusion (\n input: Scalar >> output: Scalar\n) {\n".to_string();
    for i in 0..count {
        let kind = if uniform || i % 2 == 0 {
            "math/clamp"
        } else {
            "math/scale"
        };
        source.push_str(&format!(" g{i}: {kind}\n"));
    }
    source.push_str(" input >> g0\n");
    for i in 1..count {
        source.push_str(&format!(" g{} >> g{i}\n", i - 1));
    }
    source.push_str(&format!(" g{} >> output\n}}\n", count - 1));
    f.plot = conduit_plot::parse(
        &source,
        &conduit_semantic_catalog::standard_profile_catalog(),
    )
    .unwrap();
    let host = &f.hosts[0];
    let choices: BTreeMap<_, _> = f
        .plot
        .gears
        .iter()
        .map(|gear| {
            let capability = host
                .capabilities
                .iter()
                .find(|cap| {
                    cap.kind_id == gear.kind_id
                        && cap.kind_contract_revision == gear.kind_contract_revision
                        && cap.checked_front() == gear.checked_front()
                })
                .unwrap();
            (
                gear.gear_id.clone(),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: capability.capability_id.clone(),
                },
            )
        })
        .collect();
    observations.data_flow.source_gear_id = GearId::from("fusion/g0");
    observations.transports.clear();
    observations.realization_work = choices
        .iter()
        .map(|(gear, placement)| RealizationWorkObservation {
            gear_id: gear.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: placement.capability_id.clone(),
            work_units: 100,
            provenance: fixture::provenance(&format!("work/{}", gear.as_str())),
        })
        .collect();
    observations.local_cords = f
        .plot
        .connections
        .iter()
        .map(|cord| LocalCordObservation {
            source_gear_id: cord.source_gear_id.clone(),
            sink_gear_id: cord.sink_gear_id.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            work_units: 10,
            provenance: fixture::provenance(&format!("cord/{}", cord.source_gear_id.as_str())),
        })
        .collect();
    let mut offered = fixture::offer(&f);
    offered.gear_ids = f.plot.gears.iter().map(|g| g.gear_id.clone()).collect();
    offered.internal_cords = f
        .plot
        .connections
        .iter()
        .map(|c| (c.source_gear_id.clone(), c.sink_gear_id.clone()))
        .collect();
    let candidates: Vec<_> = ["unfused", "fused"]
        .into_iter()
        .map(|id| FusionCandidate {
            candidate_id: id.into(),
            realization: LocalityCandidate {
                candidate_id: id.into(),
                placements: PlacementChoices {
                    by_gear: choices.clone(),
                },
                lines: BTreeMap::new(),
            },
            fusion_ids: if id == "fused" {
                vec![offered.fusion_id.clone()]
            } else {
                vec![]
            },
        })
        .collect();
    let selection = select_fusion_candidate(
        &f.plot,
        &f.hosts,
        &candidates,
        &observations,
        FusionPlanningInputs {
            semantic_kinds: &f.semantic_kinds,
            offers: &[offered],
            observations: &[fixture::fusion_observation(1)],
            boundaries: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    // One Gear has no internal dispatch boundary to combine; its ordinary
    // realization remains admitted rather than inventing a singleton fusion.
    assert_eq!(
        selection.selected_candidate_id,
        if count == 1 { "unfused" } else { "fused" },
        "{count}: {:?}",
        selection.considered
    );
    let planned = plan_selected_optimization(
        &f.plot,
        &f.hosts,
        &selection,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )?;
    assert!(planned.verify());
    assert_eq!(planned.plan.checked_plot_id, f.plot.checked_plot_id);
    let fragment = &planned.plan.fragments[0];
    assert_eq!(fragment.placements.len(), count);
    assert_eq!(fragment.connections.len(), count - 1);
    for gear in &f.plot.gears {
        assert!(fragment
            .placements
            .iter()
            .any(|p| p.gear_id == gear.gear_id));
    }
    assert_eq!(fragment.execution_fusions.len(), usize::from(count > 1));
    if count > 1 {
        assert_eq!(planned.fusion_groups[0].preserved_gear_ids.len(), count);
        assert_eq!(planned.fusion_groups[0].preserved_cords.len(), count - 1);
    }
    Ok(planned)
}

#[test]
fn one_eight_thirty_two_gear_chains_preserve_exact_plan_members_and_cords() {
    for count in [1, 8, 32] {
        plan_chain(count, false).unwrap();
    }
}

#[test]
fn fusion_cannot_waive_the_installed_capability_instance_limit() {
    assert!(
        matches!(plan_chain(32, true), Err(PlannerError::CapabilityInstanceLimitExceeded(reason))
        if reason.contains("max 16"))
    );
}
