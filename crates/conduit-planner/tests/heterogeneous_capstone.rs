use std::collections::BTreeMap;

use conduit_core::{
    BootId, CapabilityId, CheckedFormId, GearId, HostId, ImplementationId, OfferGeneration, PlanId,
    ResourcePoolId, SignId,
};
use conduit_planner::{
    assess_scoped_degradation, evaluate_heterogeneous_capstone, plan_cold,
    select_accelerator_candidate, select_performance_candidate, AcceleratorCandidate,
    AcceleratorDemand, AcceleratorDimension, AcceleratorObservation, AcceleratorOffer,
    AcceleratorPlanningBasis, CandidateEvaluation, CandidateEvaluationDisposition,
    CandidateStructure, CapstoneDecision, CapstoneDeviceClass, CapstoneDeviceDisposition,
    CapstoneMeasurement, DegradationInput, ExecutionMechanism, FactDomain,
    HeterogeneousCapstoneEvidence, IncrementalPlanner, ObservationProvenance, PerformanceCandidate,
    PerformanceIntent, PerformancePolicy, PerformanceProfileObservation, PlanningFact,
    PlanningFactKey, PolicyScope, PolicySourceId, PolicySourceRevision, SchedulerProofClass,
    SchedulerStrategy, StabilityPolicy,
};

fn provenance(identity: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(identity),
        source: "deterministic heterogeneous capstone fixture".into(),
        observed_at_ms: 900,
        valid_until_ms: 1_100,
    }
}

fn dimensions(memory: u64, queues: u64) -> BTreeMap<AcceleratorDimension, u64> {
    BTreeMap::from([
        (AcceleratorDimension::from("device-memory-bytes"), memory),
        (AcceleratorDimension::from("concurrent-queues"), queues),
    ])
}

fn accelerator_selection() -> conduit_planner::AcceleratorSelection {
    let mechanism = ExecutionMechanism::Accelerator {
        host_id: HostId::from("host/gpu"),
        boot_id: BootId::from("boot/gpu-1"),
        offer_generation: OfferGeneration(1),
        capability_id: CapabilityId::from("capability/heavy-inference"),
        implementation_id: ImplementationId::from("implementation/heavy-inference-gpu@1"),
        pool_id: ResourcePoolId::from("pool/gpu-0"),
        resource_generation: 7,
        residency_artifact: Some("artifact/model-a".into()),
    };
    let candidates = vec![
        AcceleratorCandidate {
            candidate_id: "heavy-on-cpu".into(),
            demands: vec![AcceleratorDemand {
                gear_id: GearId::from("mixed/heavy"),
                mechanism: ExecutionMechanism::Cpu,
                dimensions: BTreeMap::new(),
            }],
            compute_work_units: 1_000,
            transfer_work_units: 0,
            setup_work_units: 0,
        },
        AcceleratorCandidate {
            candidate_id: "heavy-on-gpu".into(),
            demands: vec![AcceleratorDemand {
                gear_id: GearId::from("mixed/heavy"),
                mechanism,
                dimensions: dimensions(8_000, 1),
            }],
            compute_work_units: 100,
            transfer_work_units: 50,
            setup_work_units: 30,
        },
    ];
    let basis = AcceleratorPlanningBasis {
        now_ms: 1_000,
        residency_credit_work_units: 25,
        offers: vec![AcceleratorOffer {
            host_id: HostId::from("host/gpu"),
            boot_id: BootId::from("boot/gpu-1"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("capability/heavy-inference"),
            implementation_id: ImplementationId::from("implementation/heavy-inference-gpu@1"),
            pool_id: ResourcePoolId::from("pool/gpu-0"),
            capacities: dimensions(16_000, 2),
        }],
        observations: vec![AcceleratorObservation {
            host_id: HostId::from("host/gpu"),
            boot_id: BootId::from("boot/gpu-1"),
            offer_generation: OfferGeneration(1),
            pool_id: ResourcePoolId::from("pool/gpu-0"),
            resource_generation: 7,
            runtime_usable: true,
            unreserved: dimensions(16_000, 2),
            resident_artifacts: vec!["artifact/model-a".into()],
            provenance: provenance("sign/gpu-current"),
        }],
    };
    select_accelerator_candidate(&candidates, &basis).unwrap()
}

fn performance_candidate(
    identity: &str,
    host: &str,
    latency: u64,
    throughput: u64,
) -> PerformanceCandidate {
    PerformanceCandidate {
        candidate_id: identity.into(),
        selected_hosts: vec![HostId::from(host)],
        profile: PerformanceProfileObservation {
            candidate_id: identity.into(),
            startup_us: 10,
            item_latency_us: latency,
            throughput_items_per_second: throughput,
            jitter_us: 2,
            bounded_response_us: None,
            transport_work_units: u64::from(host != "host/browser") * 100,
            compute_work_units: 100,
            provenance: provenance(&format!("sign/performance/{identity}")),
        },
    }
}

fn performance_policy(intent: PerformanceIntent) -> PerformancePolicy {
    PerformancePolicy {
        source: PolicySourceRevision {
            source_id: PolicySourceId::from("policy/capstone-performance"),
            revision: 1,
            scope: PolicyScope::BodyWake,
        },
        intent,
        maximum_startup_us: None,
        maximum_item_latency_us: None,
        minimum_throughput_items_per_second: None,
        maximum_jitter_us: None,
        maximum_bounded_response_us: None,
    }
}

fn performance_selections() -> (String, String, u64, u64) {
    let candidates = vec![
        performance_candidate("screen-here", "host/browser", 25, 100),
        performance_candidate("screen-on-gpu", "host/gpu", 80, 2_000),
        performance_candidate("batch-on-laptops", "host/laptops", 120, 3_000),
    ];
    let interactive = select_performance_candidate(
        CheckedFormId::from("checked/mixed-capstone"),
        &candidates,
        &performance_policy(PerformanceIntent::Interactive),
        1_000,
    )
    .unwrap();
    let batch = select_performance_candidate(
        CheckedFormId::from("checked/mixed-capstone"),
        &candidates,
        &performance_policy(PerformanceIntent::ThroughputBatch),
        1_000,
    )
    .unwrap();
    let profile = |selection: &conduit_planner::PerformancePolicySelection| {
        selection
            .considered
            .iter()
            .find(|candidate| candidate.candidate_id == selection.selected_candidate_id)
            .unwrap()
            .clone()
    };
    let interactive_latency = profile(&interactive).item_latency_us;
    let batch_throughput = profile(&batch).throughput_items_per_second;
    (
        interactive.selected_candidate_id,
        batch.selected_candidate_id,
        interactive_latency,
        batch_throughput,
    )
}

fn key(domain: FactDomain, identity: &str) -> PlanningFactKey {
    PlanningFactKey::exact(domain, identity)
}

fn fact(domain: FactDomain, identity: &str, generation: u64) -> PlanningFact {
    PlanningFact {
        key: key(domain, identity),
        generation,
        content_identity: format!("{identity}/generation/{generation}"),
    }
}

fn incremental_candidate(
    identity: &str,
    fragment: &str,
    dependency: PlanningFactKey,
    _cost: u64,
) -> CandidateStructure {
    CandidateStructure {
        candidate_id: identity.into(),
        semantic_contract_id: fragment.into(),
        implementation_family_id: format!("implementation/{identity}"),
        placement_id: format!("placement/{identity}"),
        dependencies: vec![dependency, key(FactDomain::Policy, "policy/capstone")],
    }
}

fn evaluate_incremental(
    candidate: &CandidateStructure,
    basis: &[PlanningFact],
) -> CandidateEvaluation {
    let disposition = basis.iter().find(|fact| fact.generation > 1).map_or(
        CandidateEvaluationDisposition::Admitted,
        |lost| {
            CandidateEvaluationDisposition::Rejected(format!(
                "{} generation {} unavailable",
                lost.key.identity, lost.generation
            ))
        },
    );
    CandidateEvaluation {
        disposition,
        result_identity: format!(
            "result/{}/{}",
            candidate.candidate_id,
            basis
                .iter()
                .map(|fact| fact.generation.to_string())
                .collect::<Vec<_>>()
                .join("-")
        ),
        total_cost: if candidate.candidate_id.ends_with("primary") {
            10
        } else {
            20
        },
        evaluation_work_units: 10,
    }
}

fn degradation_and_replan() -> (conduit_planner::DegradationAssessment, u64, u64, String) {
    let edge_candidates = vec![incremental_candidate(
        "edge-primary",
        "fragment/edge-filter",
        key(FactDomain::Host, "host/edge"),
        10,
    )];
    let heavy_candidates = vec![
        incremental_candidate(
            "gpu-primary",
            "fragment/heavy",
            key(FactDomain::Resource, "resource/gpu"),
            10,
        ),
        incremental_candidate(
            "cpu-fallback",
            "fragment/heavy",
            key(FactDomain::Resource, "resource/laptop-cpu"),
            20,
        ),
    ];
    let initial_facts = vec![
        fact(FactDomain::Host, "host/edge", 1),
        fact(FactDomain::Resource, "resource/gpu", 1),
        fact(FactDomain::Resource, "resource/laptop-cpu", 1),
        fact(FactDomain::Policy, "policy/capstone", 1),
    ];
    let mut edge = IncrementalPlanner::new(4).unwrap();
    let mut heavy = IncrementalPlanner::new(4).unwrap();
    edge.plan(
        &edge_candidates,
        &initial_facts,
        &StabilityPolicy::disabled(),
        evaluate_incremental,
    )
    .unwrap();
    heavy
        .plan(
            &heavy_candidates,
            &initial_facts,
            &StabilityPolicy::disabled(),
            evaluate_incremental,
        )
        .unwrap();
    let mut current = initial_facts;
    let lost = current
        .iter_mut()
        .find(|fact| fact.key.identity == "resource/gpu")
        .unwrap();
    lost.generation = 2;
    lost.content_identity = "resource/gpu/generation/2/lost".into();
    let edge_fresh = edge
        .plan(
            &edge_candidates,
            &current,
            &StabilityPolicy::disabled(),
            evaluate_incremental,
        )
        .unwrap();
    let heavy_fresh = heavy
        .plan(
            &heavy_candidates,
            &current,
            &StabilityPolicy::disabled(),
            evaluate_incremental,
        )
        .unwrap();
    let cold = plan_cold(
        &heavy_candidates,
        &current,
        &StabilityPolicy::disabled(),
        evaluate_incremental,
    )
    .unwrap();
    assert_eq!(
        heavy_fresh.selected_result_identity,
        cold.selected_result_identity
    );
    assert!(
        heavy_fresh.metrics.logical_latency_work_units < cold.metrics.logical_latency_work_units
    );
    let inputs = vec![
        DegradationInput {
            fragment_id: "fragment/edge-filter".into(),
            previous_candidate_id: "edge-primary".into(),
            candidates: edge_candidates,
            fresh_plan: Some(edge_fresh),
            refusal: None,
        },
        DegradationInput {
            fragment_id: "fragment/heavy".into(),
            previous_candidate_id: "gpu-primary".into(),
            candidates: heavy_candidates,
            fresh_plan: Some(heavy_fresh.clone()),
            refusal: None,
        },
    ];
    let degradation = assess_scoped_degradation(
        PlanId::from("plan/mixed/old"),
        Some(PlanId::from("plan/mixed/fresh")),
        &[key(FactDomain::Resource, "resource/gpu")],
        &inputs,
    )
    .unwrap();
    (
        degradation,
        heavy_fresh.metrics.logical_latency_work_units,
        cold.metrics.logical_latency_work_units,
        cold.selected_result_identity,
    )
}

fn measurement(
    strategy: SchedulerStrategy,
    latency: u64,
    throughput: u64,
    bytes: u64,
    messages: u64,
    planner: u64,
    coordination: u64,
) -> CapstoneMeasurement {
    CapstoneMeasurement {
        strategy,
        semantic_identity: "checked/mixed-capstone@sha256:01".into(),
        authority_identity: "authority/mixed-capstone@7".into(),
        resource_admission_complete: true,
        useful_work_units: 10_000,
        interactive_latency_us: latency,
        batch_throughput_items_per_second: throughput,
        line_bytes: bytes,
        line_messages: messages,
        planner_work_units: planner,
        coordination_work_units: coordination,
        scheduler_overhead_work_units: if strategy == SchedulerStrategy::Optimized {
            40
        } else {
            0
        },
        accelerator_reserved_units: if strategy == SchedulerStrategy::Optimized {
            8_000
        } else {
            0
        },
        accelerator_utilized_units: if strategy == SchedulerStrategy::Optimized {
            6_000
        } else {
            0
        },
        fusion_choices: u32::from(strategy == SchedulerStrategy::Optimized),
        placement_churn: if strategy == SchedulerStrategy::Optimized {
            1
        } else {
            4
        },
        pressure_events: 1,
        refusal_events: 1,
    }
}

fn evidence() -> HeterogeneousCapstoneEvidence {
    let exact = conduit_signal::triple::exact_plan().unwrap();
    assert!(conduit_core::verify_plan(&exact.plan));
    assert_eq!(exact.plan.fragments.len(), 3);
    let accelerator = accelerator_selection();
    assert_eq!(accelerator.selected_candidate_id, "heavy-on-gpu");
    let (interactive, batch, latency, throughput) = performance_selections();
    assert_eq!(interactive, "screen-here");
    assert_eq!(batch, "batch-on-laptops");
    let (degradation, incremental_work, cold_work, result_identity) = degradation_and_replan();
    HeterogeneousCapstoneEvidence {
        proof_class: SchedulerProofClass::DeterministicFixture,
        measurements: vec![
            measurement(
                SchedulerStrategy::Optimized,
                latency,
                throughput,
                100_000,
                1_000,
                incremental_work,
                200,
            ),
            measurement(
                SchedulerStrategy::CentralizedStrongestHost,
                80,
                2_000,
                1_000_000,
                10_000,
                cold_work,
                1_000,
            ),
            measurement(
                SchedulerStrategy::CheapestFitWithoutCoordinationCost,
                120,
                3_000,
                1_000_000,
                10_000,
                cold_work,
                800,
            ),
        ],
        devices: vec![
            CapstoneDeviceClass {
                class_id: "class/constrained-edge".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "workload/high-rate-filter".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "class/old-laptops".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "workload/batch-shards".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "class/gpu".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "workload/heavy-inference".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "class/browser".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "workload/interactive-presentation".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "class/tiny-display-only".into(),
                disposition: CapstoneDeviceDisposition::IntentionallyUnused {
                    reason: "offers neither batch, inference, nor interactive implementation"
                        .into(),
                },
            },
        ],
        decisions: vec![
            CapstoneDecision {
                workload_id: "workload/high-rate-filter".into(),
                choice: "filter-on-edge".into(),
                principal_reason: "source data is large; reduced result is ten times smaller"
                    .into(),
            },
            CapstoneDecision {
                workload_id: "workload/batch-shards".into(),
                choice: "batch-on-old-laptops".into(),
                principal_reason: "throughput policy admits aggregate general CPU capacity".into(),
            },
            CapstoneDecision {
                workload_id: "workload/heavy-inference".into(),
                choice: "inference-on-gpu".into(),
                principal_reason: "heavy work amortizes transfer and reserves exact device memory"
                    .into(),
            },
            CapstoneDecision {
                workload_id: "workload/interactive-presentation".into(),
                choice: "screen-on-browser".into(),
                principal_reason: "interactive latency policy keeps presentation human-local"
                    .into(),
            },
            CapstoneDecision {
                workload_id: "workload/local-chain".into(),
                choice: "safe-local-fusion".into(),
                principal_reason: "typed boundary is unobserved; fusion avoids queue and Line cost"
                    .into(),
            },
            CapstoneDecision {
                workload_id: "workload/bad-remote".into(),
                choice: "remote-candidate-refused".into(),
                principal_reason: "tiny compute gain does not repay transfer and pressure cost"
                    .into(),
            },
        ],
        cold_replan_result_identity: result_identity.clone(),
        incremental_replan_result_identity: result_identity,
        cold_replan_work_units: cold_work,
        incremental_replan_work_units: incremental_work,
        degradation,
    }
}

#[test]
fn mixed_heterogeneous_body_beats_both_naive_baselines_without_weakening_truth() {
    let report = evaluate_heterogeneous_capstone(evidence()).unwrap();
    assert_eq!(
        report.proof_class,
        SchedulerProofClass::DeterministicFixture
    );
    assert_eq!(report.baselines.len(), 2);
    assert!(report
        .gains_by_baseline
        .iter()
        .all(|(_, dimensions)| dimensions.len() >= 2));
    assert!(report.incremental_replan_work_units < report.cold_replan_work_units);
    assert_eq!(report.degradation.what_failed().len(), 1);
    assert_eq!(report.degradation.what_still_works().len(), 1);
    assert!(report.devices.iter().any(|device| matches!(
        device.disposition,
        CapstoneDeviceDisposition::IntentionallyUnused { .. }
    )));
}

#[test]
fn semantic_authority_admission_overhead_and_oracle_shortcuts_fail_closed() {
    let mutate = |change: fn(&mut HeterogeneousCapstoneEvidence)| {
        let mut evidence = evidence();
        change(&mut evidence);
        assert!(evaluate_heterogeneous_capstone(evidence).is_err());
    };
    mutate(|evidence| {
        evidence.measurements[1]
            .semantic_identity
            .push_str("-changed")
    });
    mutate(|evidence| {
        evidence.measurements[1]
            .authority_identity
            .push_str("-changed")
    });
    mutate(|evidence| evidence.measurements[1].resource_admission_complete = false);
    mutate(|evidence| evidence.measurements[0].scheduler_overhead_work_units = 10_000);
    mutate(|evidence| {
        evidence
            .incremental_replan_result_identity
            .push_str("-different")
    });
    mutate(|evidence| evidence.measurements[0].accelerator_utilized_units = 8_001);
    mutate(|evidence| {
        evidence
            .devices
            .retain(|device| matches!(device.disposition, CapstoneDeviceDisposition::Used { .. }))
    });
}

#[test]
fn no_single_magic_score_can_hide_loss_of_two_distinct_gains() {
    let mut evidence = evidence();
    let optimized = evidence.measurements[0].clone();
    for baseline in &mut evidence.measurements[1..] {
        baseline.interactive_latency_us = optimized.interactive_latency_us;
        baseline.batch_throughput_items_per_second = optimized.batch_throughput_items_per_second;
        baseline.line_bytes = optimized.line_bytes;
        baseline.line_messages = optimized.line_messages;
        baseline.planner_work_units = optimized.planner_work_units;
        baseline.placement_churn = optimized.placement_churn;
        baseline.coordination_work_units = optimized.coordination_work_units;
    }
    assert!(evaluate_heterogeneous_capstone(evidence).is_err());
}
