use conduit_planner::proof::heterogeneous::{
    evaluate_heterogeneous_capstone, CapstoneDecision, CapstoneDeviceClass,
    CapstoneDeviceDisposition, CapstoneMeasurement, HeterogeneousCapstoneEvidence,
    HeterogeneousCapstoneReport, SchedulerProofClass, SchedulerStrategy,
};
use conduit_planner::{DegradationAssessment, DegradationFragment, DegradationFragmentDisposition};

use crate::proof::PatchbayHeterogeneousCapstoneExplanation;

fn measurement(strategy: SchedulerStrategy, latency: u64, throughput: u64) -> CapstoneMeasurement {
    CapstoneMeasurement {
        strategy,
        semantic_identity: "checked/mixed".into(),
        authority_identity: "authority/mixed".into(),
        resource_admission_complete: true,
        useful_work_units: 1_000,
        interactive_latency_us: latency,
        batch_throughput_items_per_second: throughput,
        line_bytes: if strategy == SchedulerStrategy::Optimized {
            100
        } else {
            1_000
        },
        line_messages: if strategy == SchedulerStrategy::Optimized {
            10
        } else {
            100
        },
        planner_work_units: if strategy == SchedulerStrategy::Optimized {
            10
        } else {
            100
        },
        coordination_work_units: if strategy == SchedulerStrategy::Optimized {
            40
        } else {
            400
        },
        scheduler_overhead_work_units: if strategy == SchedulerStrategy::Optimized {
            20
        } else {
            0
        },
        accelerator_reserved_units: 8,
        accelerator_utilized_units: 6,
        fusion_choices: 1,
        placement_churn: if strategy == SchedulerStrategy::Optimized {
            1
        } else {
            4
        },
        pressure_events: 1,
        refusal_events: 1,
    }
}

fn report() -> HeterogeneousCapstoneReport {
    evaluate_heterogeneous_capstone(HeterogeneousCapstoneEvidence {
        proof_class: SchedulerProofClass::DeterministicFixture,
        measurements: vec![
            measurement(SchedulerStrategy::Optimized, 10, 1_000),
            measurement(SchedulerStrategy::CentralizedStrongestHost, 50, 800),
            measurement(
                SchedulerStrategy::CheapestFitWithoutCoordinationCost,
                70,
                1_000,
            ),
        ],
        devices: vec![
            CapstoneDeviceClass {
                class_id: "edge".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "filter".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "laptop".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "batch".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "gpu".into(),
                disposition: CapstoneDeviceDisposition::Used {
                    workload_id: "infer".into(),
                },
            },
            CapstoneDeviceClass {
                class_id: "display".into(),
                disposition: CapstoneDeviceDisposition::IntentionallyUnused {
                    reason: "no fitting work".into(),
                },
            },
        ],
        decisions: vec![CapstoneDecision {
            workload_id: "infer".into(),
            choice: "gpu".into(),
            principal_reason: "accelerator admission".into(),
        }],
        cold_replan_result_identity: "plan/fresh".into(),
        incremental_replan_result_identity: "plan/fresh".into(),
        cold_replan_work_units: 20,
        incremental_replan_work_units: 5,
        degradation: DegradationAssessment {
            previous_plan_id: "plan/old".into(),
            replacement_plan_id: Some("plan/fresh".into()),
            fragments: vec![
                DegradationFragment {
                    fragment_id: "filter".into(),
                    previous_candidate_id: "edge".into(),
                    changed_dependencies: vec![],
                    disposition: DegradationFragmentDisposition::StillWorks,
                    reused_unaffected_structure: true,
                },
                DegradationFragment {
                    fragment_id: "infer".into(),
                    previous_candidate_id: "gpu".into(),
                    changed_dependencies: vec![],
                    disposition: DegradationFragmentDisposition::Replaced {
                        candidate_id: "cpu".into(),
                    },
                    reused_unaffected_structure: false,
                },
            ],
            automatic_retry_count: 0,
        },
    })
    .unwrap()
}

#[test]
fn patchbay_keeps_measurements_decisions_and_proof_class_distinct() {
    let explanation = PatchbayHeterogeneousCapstoneExplanation::from_report(&report()).unwrap();
    assert_eq!(explanation.baselines.len(), 2);
    assert!(explanation
        .baselines
        .iter()
        .all(|baseline| baseline.gains.len() >= 2));
    assert!(explanation.proof_class.contains("not physical or HIL"));
    assert!(!explanation.physical_evidence_claimed);
    assert_eq!(explanation.intentionally_unused_devices.len(), 1);
    assert_eq!(explanation.partial_failure.len(), 2);
}
