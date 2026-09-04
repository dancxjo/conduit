use conduit_core::{PlanId, SignId};
use conduit_planner::proof::voyager::{
    VoyagerCapstoneEvidence, VoyagerProofClass, VoyagerScarKind, VoyagerStageEvidence,
    VoyagerStageMetrics,
};
use patchbay_model::proof::explain_voyager_capstone;

fn metrics(index: u16) -> VoyagerStageMetrics {
    VoyagerStageMetrics {
        surviving_hosts: 8 - index.min(5),
        surviving_bases: 7 - index.min(5),
        surviving_lines: 6 - index.min(4),
        full_capabilities: 4_u16.saturating_sub(index / 2),
        degraded_capabilities: u16::from(index >= 3),
        unavailable_capabilities: u16::from(index >= 7),
        realization_gears: 3 + index,
        realization_depth: 1 + index / 2,
        line_hops: index,
        admitted_line_bytes: u64::from(index) * 64,
        estimated_item_latency_us: u64::from(index + 1) * 5_000,
        planning_work: u32::from(index + 1),
        incrementally_reused_candidates: index / 2,
        reserved_resource_units: 8,
        admitted_sessions: 1,
    }
}

#[test]
fn patchbay_explains_each_scar_without_flattening_the_timeline_to_healed() {
    let scars = [
        VoyagerScarKind::HealthyPreferred,
        VoyagerScarKind::ExactRedundancy,
        VoyagerScarKind::MechanismReroute,
        VoyagerScarKind::LinePathReroute,
        VoyagerScarKind::ExplicitDegradation {
            profile_id: "profile/survival".into(),
        },
        VoyagerScarKind::DormantReadmission {
            host_id: "host/legacy".into(),
        },
        VoyagerScarKind::RecursiveRecovery {
            semantic_profile: "navigation/full".into(),
        },
        VoyagerScarKind::SurvivalPolicy {
            policy_id: "policy/voyager/survival@1".into(),
        },
        VoyagerScarKind::Irrecoverable {
            requirement_id: "control/authorized-effect".into(),
        },
    ];
    let stages = scars
        .into_iter()
        .enumerate()
        .map(|(index, scar)| VoyagerStageEvidence {
            stage_id: format!("stage-{index}"),
            observation_generation: (index + 1) as u64,
            observation_signs: vec![SignId::from(format!("sign/stage-{index}"))],
            failed_facts: (index > 0)
                .then(|| format!("failed/fact-{index}"))
                .into_iter()
                .collect(),
            plan_id: (index < 8).then(|| PlanId::from(format!("plan/stage-{index}"))),
            host_ids: vec![format!("host/stage-{index}")],
            implementation_ids: vec![format!("implementation/stage-{index}")],
            line_ids: (index > 0)
                .then(|| format!("line/stage-{index}"))
                .into_iter()
                .collect(),
            resource_binding_count: 1,
            authority_binding_count: 1,
            metrics: metrics(index as u16),
            scars: vec![scar],
        })
        .collect::<Vec<_>>();
    let evidence = VoyagerCapstoneEvidence {
        proof_class: VoyagerProofClass::DeterministicCiFixture,
        stages,
        historical_plan_ids: (0..8)
            .map(|index| PlanId::from(format!("plan/stage-{index}")))
            .collect(),
        observation_sign_count: 9,
        final_metrics: metrics(8),
        exact_redundancy_observed: true,
        mechanism_diversity_observed: true,
        line_path_diversity_observed: true,
        explicit_degradation_observed: true,
        dormant_readmission_observed: true,
        recursive_recovery_observed: true,
        irrecoverability_observed: true,
        normal_survival_divergence_observed: true,
    };
    let explanation = explain_voyager_capstone(&evidence).unwrap();
    assert_eq!(
        explanation.proof_class,
        "deterministic-ci-fixture (not physical/HIL)"
    );
    assert_eq!(explanation.stages.len(), 9);
    assert!(explanation.stages[4].what_is_degraded[0].contains("profile/survival"));
    assert!(explanation.stages[5].what_old_equipment_reentered[0].contains("host/legacy"));
    assert!(explanation.stages[6].what_realization_expanded[0].contains("navigation/full"));
    assert!(explanation.stages[2]
        .lines_carrying_work
        .contains(&"line/stage-2".to_string()));
    assert_eq!(
        explanation.stages[8].what_remains_impossible,
        ["control/authorized-effect"]
    );
    assert!(explanation.summary.contains("immutable historical Plans"));
    assert!(explanation.summary.contains("true irrecoverability"));
    assert!(!explanation.summary.contains("HEALED"));
}
