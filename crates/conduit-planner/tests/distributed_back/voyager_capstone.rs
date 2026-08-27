use super::*;
use conduit_core::{AuthorityGrantId, CharacteristicId, GearId, ImplementationId, SignId};
use conduit_planner::proof::voyager::{
    prove_voyager_capstone, VoyagerBodyInventory, VoyagerCapstoneRefusal, VoyagerDamageStage,
    VoyagerIrrecoverableReason, VoyagerPhenomenon, VoyagerProofClass, VoyagerStageMetrics,
};
use conduit_planner::{
    prove_recursive_recovery, select_plan_with_survival_policy, triage_scarce_resource,
    CurrentDormantCandidate, DegradedDimensionEvidence, DiversityRelationship,
    DiversityReplacementEvidence, DormantReadmissionEvidence, ExplicitCriticality,
    PlannerFactValue, PreviousPlanDisposition, RecursiveRecoveryCandidate, RecursiveRecoveryLimits,
    ServiceProfileAdmission, ServiceProfileDisposition, SurvivalCandidate,
    SurvivalCandidateDisposition, SurvivalPlanningMode, SurvivalPlanningPolicy,
    SurvivalPolicyRefusal, SurvivalTradeoff, WorkloadResourceRequest,
};

const PROFILE: &str = "test/provider-generate@full-v1";

fn inventory() -> VoyagerBodyInventory {
    VoyagerBodyInventory {
        general_purpose_hosts: 3,
        accelerator_hosts: 1,
        constrained_hosts: 3,
        sensor_input_capabilities: 2,
        presentation_capabilities: 2,
        line_mechanisms: 4,
        dormant_equipment: 1,
        recursive_realization_families: 1,
    }
}

fn metrics(step: u16, plan: Option<&conduit_core::Plan>) -> VoyagerStageMetrics {
    let placements = plan
        .into_iter()
        .flat_map(|plan| &plan.fragments)
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    let connections = plan
        .into_iter()
        .flat_map(|plan| &plan.fragments)
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    VoyagerStageMetrics {
        surviving_hosts: 8 - step.min(5),
        surviving_bases: 7 - step.min(5),
        surviving_lines: 6 - step.min(4),
        full_capabilities: 4_u16.saturating_sub(step / 2),
        degraded_capabilities: u16::from(step >= 3),
        unavailable_capabilities: u16::from(step >= 7),
        realization_gears: u16::try_from(placements.len()).unwrap_or(1).max(1),
        realization_depth: 1 + step / 2,
        line_hops: connections
            .iter()
            .map(|connection| connection.admitted_lines.len())
            .sum::<usize>()
            .try_into()
            .unwrap_or(0),
        admitted_line_bytes: connections
            .iter()
            .map(|connection| u64::from(connection.byte_capacity))
            .sum(),
        estimated_item_latency_us: u64::from(step + 1) * 5_000,
        planning_work: u32::from(step + 1) * 3,
        incrementally_reused_candidates: step / 2,
        reserved_resource_units: placements
            .iter()
            .flat_map(|placement| &placement.resources)
            .map(|binding| u64::from(binding.units))
            .sum(),
        admitted_sessions: u16::try_from(connections.len()).unwrap_or(1).max(1),
    }
}

fn degradation(plan: &conduit_core::Plan) -> ServiceProfileAdmission {
    let placement = &plan.fragments[0].placements[0];
    ServiceProfileAdmission {
        disposition: ServiceProfileDisposition::Degraded,
        profile_id: "test/provider-generate@degraded-v1".into(),
        policy_id: Some("policy/voyager/reviewed-degradation@1".into()),
        policy_revision: Some(1),
        choice: conduit_planner::PlacementChoice {
            host_id: placement.host_id.clone(),
            capability_id: placement.capability_id.clone(),
        },
        decisions: vec![],
        dimensions: vec![DegradedDimensionEvidence {
            characteristic_id: CharacteristicId::from("test/update-rate@1"),
            human_name: "update rate".into(),
            requested_value: PlannerFactValue::Category("full".into()),
            weakest_permitted_value: PlannerFactValue::Category("survival".into()),
            admitted_value: PlannerFactValue::Category("survival".into()),
        }],
        observation_signs: vec![SignId::from("sign/degraded/current")],
    }
}

fn dormant(previous: &conduit_core::Plan, plan: &conduit_core::Plan) -> DormantReadmissionEvidence {
    let placement = &plan.fragments[0].placements[0];
    DormantReadmissionEvidence {
        previous_plan_id: previous.plan_id.clone(),
        plan_id: plan.plan_id.clone(),
        candidate: CurrentDormantCandidate {
            body_membership_id: "body/voyager/legacy-host".into(),
            gear_id: placement.gear_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            offer_generation: placement.offer_generation,
            capability_id: placement.capability_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            required_lines: vec![],
            resource_observation_signs: vec![SignId::from("sign/legacy/resource/current")],
            line_observation_signs: vec![SignId::from("sign/legacy/line/current")],
            authority_grant_ids: vec![AuthorityGrantId::from("grant/legacy/current")],
            unused_before: true,
            available_now: true,
        },
        selected_because_preferred_path_is_gone: true,
        historical_boot_reused: false,
        historical_authority_restored: false,
    }
}

fn recursive(
    previous: &conduit_core::Plan,
    plan: &conduit_core::Plan,
) -> conduit_planner::RecursiveRecoveryEvidence {
    prove_recursive_recovery(
        &RecursiveRecoveryCandidate {
            lost_direct_plan: previous,
            replacement_plan: plan,
            required_semantic_profile: PROFILE,
            offered_semantic_profile: PROFILE,
            offered_profile_is_reviewed_degradation: false,
            direct_implementation_unavailable: true,
            all_host_reservations_admitted: true,
            all_required_authority_admitted: true,
            expansion_depth: 3,
            search_work: 12,
            candidates_considered: 4,
            estimated_item_latency_us: 40_000,
        },
        RecursiveRecoveryLimits {
            maximum_depth: 4,
            maximum_work: 32,
            maximum_candidates: 8,
            maximum_gears: 16,
            maximum_remote_connections: 12,
            maximum_item_latency_us: 50_000,
        },
    )
    .unwrap()
}

fn survival(
    previous: &conduit_core::Plan,
    plan: &conduit_core::Plan,
) -> conduit_planner::SurvivalPlanSelection {
    select_plan_with_survival_policy(
        PROFILE,
        &[
            SurvivalCandidate {
                plan: previous,
                semantic_profile: PROFILE,
                disposition: SurvivalCandidateDisposition::FullyCompatible,
                current: true,
                currently_available: false,
                authority_admitted: true,
                all_host_reservations_admitted: true,
                unavailable_prerequisites: 1,
                shared_dependency_exposures: 3,
                hop_count: 1,
                estimated_item_latency_us: 1_000,
                resource_units: 4,
            },
            SurvivalCandidate {
                plan,
                semantic_profile: PROFILE,
                disposition: SurvivalCandidateDisposition::FullyCompatible,
                current: false,
                currently_available: true,
                authority_admitted: true,
                all_host_reservations_admitted: true,
                unavailable_prerequisites: 0,
                shared_dependency_exposures: 1,
                hop_count: 5,
                estimated_item_latency_us: 40_000,
                resource_units: 80,
            },
        ],
        &SurvivalPlanningPolicy {
            policy_id: "policy/voyager/survival@1".into(),
            revision: 1,
            mode: SurvivalPlanningMode::Survival,
            normal_maximum_hops: 2,
            normal_maximum_latency_us: 10_000,
            admit_costly_full_profile: true,
            admit_reviewed_degradation: false,
            tradeoffs: vec![
                SurvivalTradeoff::PreferFullProfile,
                SurvivalTradeoff::MinimizeSharedDependencyExposure,
            ],
        },
    )
    .unwrap()
}

#[test]
fn progressive_damage_exposes_every_truthful_voyager_phenomenon() {
    let (_, p0) = direct_plan_on("preferred-accelerator");
    let (_, p1) = direct_plan_on("equivalent-redundant");
    let (_, p2) = plan_with_http_part(host("diverse", &[HTTP, DECODE]));
    let (_, p3) = plan_with_http_part(host("degraded", &[HTTP, DECODE]));
    let (_, p4) = plan_with_http_part(host("legacy", &[HTTP, DECODE]));
    let (_, p5) = plan_with_http_part(host("recursive", &[HTTP, DECODE]));
    let (_, p6) = plan_with_http_part(host("survival", &[HTTP, DECODE]));
    let plan_ids = [
        p0.plan_id.clone(),
        p1.plan_id.clone(),
        p2.plan_id.clone(),
        p3.plan_id.clone(),
        p4.plan_id.clone(),
        p5.plan_id.clone(),
        p6.plan_id.clone(),
    ];
    let signs = (0..8)
        .map(|index| vec![SignId::from(format!("sign/voyager/stage-{index}/fresh"))])
        .collect::<Vec<_>>();
    let diverse = DiversityReplacementEvidence {
        semantic_capability_id: PROFILE.into(),
        semantic_cord_id: "cord/voyager/control".into(),
        previous_candidate_id: "candidate/direct".into(),
        replacement_candidate_id: "candidate/distributed".into(),
        previous_plan_id: p1.plan_id.clone(),
        replacement_plan_id: p2.plan_id.clone(),
        previous_plan_disposition: PreviousPlanDisposition::InvalidatedRequiresTermination,
        unavailable_previous_dependencies: vec![conduit_planner::PlanningFactKey::exact(
            conduit_planner::FactDomain::Implementation,
            "accelerator/lost",
        )],
        relationship: DiversityRelationship::MechanismAndLinePathDiverse,
        replacement_mechanisms: vec![conduit_planner::MechanismDependency {
            gear_id: GearId::from("distributed/generate/request"),
            implementation_id: ImplementationId::from("test/diverse/http@1"),
        }],
        replacement_line_path: vec![conduit_planner::LinePathHop {
            connection_id: conduit_core::ConnectionId::from("distributed/control"),
            line_id: conduit_core::LineId::from("line/surviving-optical"),
            base_instance_id: conduit_core::BaseInstanceId::from("base/surviving-optical"),
        }],
    };
    let stages = vec![
        VoyagerDamageStage {
            stage_id: "healthy",
            observation_generation: 1,
            observation_signs: &signs[0],
            failed_facts: &[],
            plan: Some(&p0),
            previous_plan_id: None,
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(0, Some(&p0)),
            phenomena: vec![VoyagerPhenomenon::HealthyPreferred {
                retained_dominated_families: 3,
                activated_dominated_families: 0,
            }],
        },
        VoyagerDamageStage {
            stage_id: "accelerator-loss-redundancy",
            observation_generation: 2,
            observation_signs: &signs[1],
            failed_facts: &["accelerator/preferred"],
            plan: Some(&p1),
            previous_plan_id: Some(plan_ids[0].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(1, Some(&p1)),
            phenomena: vec![VoyagerPhenomenon::ExactRedundantReplacement {
                previous_plan_id: plan_ids[0].clone(),
            }],
        },
        VoyagerDamageStage {
            stage_id: "mechanism-and-path-reroute",
            observation_generation: 3,
            observation_signs: &signs[2],
            failed_facts: &["line/primary", "implementation/accelerated"],
            plan: Some(&p2),
            previous_plan_id: Some(plan_ids[1].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(2, Some(&p2)),
            phenomena: vec![VoyagerPhenomenon::DiverseReplacement(diverse)],
        },
        VoyagerDamageStage {
            stage_id: "explicit-degradation",
            observation_generation: 4,
            observation_signs: &signs[3],
            failed_facts: &["sensor/full-rate"],
            plan: Some(&p3),
            previous_plan_id: Some(plan_ids[2].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(3, Some(&p3)),
            phenomena: vec![VoyagerPhenomenon::ExplicitDegradation {
                plan_id: p3.plan_id.clone(),
                admission: degradation(&p3),
            }],
        },
        VoyagerDamageStage {
            stage_id: "legacy-readmission",
            observation_generation: 5,
            observation_signs: &signs[4],
            failed_facts: &["host/general-purpose-a"],
            plan: Some(&p4),
            previous_plan_id: Some(plan_ids[3].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(4, Some(&p4)),
            phenomena: vec![VoyagerPhenomenon::DormantReadmission(dormant(&p3, &p4))],
        },
        VoyagerDamageStage {
            stage_id: "recursive-recomposition",
            observation_generation: 6,
            observation_signs: &signs[5],
            failed_facts: &["implementation/direct"],
            plan: Some(&p5),
            previous_plan_id: Some(plan_ids[4].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(5, Some(&p5)),
            phenomena: vec![VoyagerPhenomenon::RecursiveRecovery {
                lost_plan_id: p4.plan_id.clone(),
                evidence: recursive(&p4, &p5),
            }],
        },
        VoyagerDamageStage {
            stage_id: "survival-policy",
            observation_generation: 7,
            observation_signs: &signs[6],
            failed_facts: &["line/secondary"],
            plan: Some(&p6),
            previous_plan_id: Some(plan_ids[5].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(6, Some(&p6)),
            phenomena: vec![VoyagerPhenomenon::SurvivalPolicyDecision {
                truth_generation: 7,
                normal_refusal: SurvivalPolicyRefusal::NormalCostEnvelopeExceeded,
                survival_selection: survival(&p5, &p6),
                scarce_resource_triage: triage_scarce_resource(
                    4,
                    &[
                        WorkloadResourceRequest {
                            workload_id: "presentation/nonessential".into(),
                            resource_units: 4,
                            criticality: ExplicitCriticality::Deferrable,
                            policy_source_id: "policy/voyager/operator-reviewed@1".into(),
                            policy_source_revision: 1,
                        },
                        WorkloadResourceRequest {
                            workload_id: "control/required".into(),
                            resource_units: 4,
                            criticality: ExplicitCriticality::Essential,
                            policy_source_id: "policy/voyager/operator-reviewed@1".into(),
                            policy_source_revision: 1,
                        },
                    ],
                )
                .unwrap(),
                hard_failure_refused_under_both: true,
            }],
        },
        VoyagerDamageStage {
            stage_id: "irrecoverable-final-scar",
            observation_generation: 8,
            observation_signs: &signs[7],
            failed_facts: &["authority/controller-destroyed"],
            plan: None,
            previous_plan_id: Some(plan_ids[6].clone()),
            required_authority_admitted: true,
            finite_resource_and_session_admission: true,
            metrics: metrics(7, None),
            phenomena: vec![VoyagerPhenomenon::Irrecoverable {
                requirement_id: "control/final-authorized-effect".into(),
                reason: VoyagerIrrecoverableReason::AuthorityUnavailable,
            }],
        },
    ];

    let evidence = prove_voyager_capstone(
        VoyagerProofClass::DeterministicCiFixture,
        &inventory(),
        &stages,
    )
    .unwrap();
    assert_eq!(
        evidence.proof_class,
        VoyagerProofClass::DeterministicCiFixture
    );
    assert_eq!(evidence.stages.len(), 8);
    assert_eq!(evidence.historical_plan_ids, plan_ids);
    assert!(evidence.exact_redundancy_observed);
    assert!(evidence.mechanism_diversity_observed);
    assert!(evidence.line_path_diversity_observed);
    assert!(evidence.explicit_degradation_observed);
    assert!(evidence.dormant_readmission_observed);
    assert!(evidence.recursive_recovery_observed);
    assert!(evidence.irrecoverability_observed);
    assert!(evidence.normal_survival_divergence_observed);
    assert!(!evidence.stages[2].line_ids.is_empty());
    assert_eq!(
        evidence.stages[2].metrics.realization_gears,
        p2.fragments
            .iter()
            .map(|fragment| fragment.placements.len())
            .sum::<usize>() as u16
    );

    let mut bypassed = stages.clone();
    bypassed[6].required_authority_admitted = false;
    assert_eq!(
        prove_voyager_capstone(
            VoyagerProofClass::DeterministicCiFixture,
            &inventory(),
            &bypassed
        ),
        Err(VoyagerCapstoneRefusal::AuthorityOrAdmissionBypassed)
    );
    let mut stale = stages.clone();
    stale[4].observation_signs = &signs[3];
    assert_eq!(
        prove_voyager_capstone(
            VoyagerProofClass::DeterministicCiFixture,
            &inventory(),
            &stale
        ),
        Err(VoyagerCapstoneRefusal::MissingFreshObservation)
    );
    let mut broken_history = stages.clone();
    broken_history[2].previous_plan_id = Some(p0.plan_id.clone());
    assert_eq!(
        prove_voyager_capstone(
            VoyagerProofClass::DeterministicCiFixture,
            &inventory(),
            &broken_history
        ),
        Err(VoyagerCapstoneRefusal::InvalidPlanHistory)
    );
    assert_ne!(
        VoyagerProofClass::DeterministicCiFixture,
        VoyagerProofClass::PhysicalHil
    );
}
