use conduit_core::{CheckedFormId, HostId, SignId};
use conduit_planner::{
    select_performance_candidate, ObservationProvenance, PerformanceCandidate,
    PerformanceCandidateDisposition, PerformanceIntent, PerformancePolicy,
    PerformanceProfileObservation, PolicyScope, PolicySourceId, PolicySourceRevision,
};

fn policy(intent: PerformanceIntent) -> PerformancePolicy {
    PerformancePolicy {
        source: PolicySourceRevision {
            source_id: PolicySourceId::from("operator-performance-policy"),
            revision: 3,
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

#[allow(clippy::too_many_arguments)]
fn candidate(
    id: &str,
    hosts: &[&str],
    startup_us: u64,
    latency_us: u64,
    throughput: u64,
    jitter_us: u64,
    bounded_response_us: Option<u64>,
    transport: u64,
    compute: u64,
) -> PerformanceCandidate {
    PerformanceCandidate {
        candidate_id: id.to_string(),
        selected_hosts: hosts.iter().copied().map(HostId::from).collect(),
        profile: PerformanceProfileObservation {
            candidate_id: id.to_string(),
            startup_us,
            item_latency_us: latency_us,
            throughput_items_per_second: throughput,
            jitter_us,
            bounded_response_us,
            transport_work_units: transport,
            compute_work_units: compute,
            provenance: ObservationProvenance {
                sign_id: SignId::from(format!("profile-{id}")),
                source: "reviewed deterministic fixture".to_string(),
                observed_at_ms: 900,
                valid_until_ms: 1_100,
            },
        },
    }
}

fn fixtures() -> Vec<PerformanceCandidate> {
    vec![
        candidate("local-cpu", &["input-laptop"], 10, 25, 100, 4, None, 0, 300),
        candidate(
            "remote-gpu",
            &["gpu-workstation"],
            400,
            80,
            2_000,
            20,
            None,
            150,
            40,
        ),
        candidate(
            "laptop-shards",
            &["old-laptop-a", "old-laptop-b", "old-laptop-c"],
            200,
            120,
            3_000,
            30,
            None,
            90,
            120,
        ),
        candidate(
            "local-control",
            &["microcontroller"],
            5,
            30,
            80,
            2,
            Some(75),
            0,
            180,
        ),
    ]
}

#[test]
fn one_form_identity_gets_distinct_rational_plans_from_explicit_policy() {
    let form_id = CheckedFormId::from("same-semantic-form");
    let interactive = select_performance_candidate(
        form_id.clone(),
        &fixtures(),
        &policy(PerformanceIntent::Interactive),
        1_000,
    )
    .expect("interactive policy selects current evidence");
    let batch = select_performance_candidate(
        form_id.clone(),
        &fixtures(),
        &policy(PerformanceIntent::ThroughputBatch),
        1_000,
    )
    .expect("batch policy selects current evidence");

    assert_eq!(interactive.checked_form_id, form_id);
    assert_eq!(batch.checked_form_id, interactive.checked_form_id);
    assert_eq!(interactive.selected_candidate_id, "local-cpu");
    assert_eq!(batch.selected_candidate_id, "laptop-shards");
    assert_eq!(
        batch
            .considered
            .iter()
            .find(|item| item.disposition == PerformanceCandidateDisposition::Selected)
            .expect("batch winner exists")
            .selected_hosts
            .len(),
        3
    );
}

#[test]
fn interactive_prefers_latency_over_remote_peak_compute() {
    let selection = select_performance_candidate(
        CheckedFormId::from("interactive-form"),
        &fixtures(),
        &policy(PerformanceIntent::Interactive),
        1_000,
    )
    .expect("explicit interactive policy is evaluated");
    assert_eq!(selection.selected_candidate_id, "local-cpu");
    assert!(selection.explain().contains("operator-performance-policy"));
    assert!(selection.explain().contains("Interactive"));
}

#[test]
fn batch_can_prefer_aggregate_throughput_across_old_hosts() {
    let selection = select_performance_candidate(
        CheckedFormId::from("batch-form"),
        &fixtures(),
        &policy(PerformanceIntent::ThroughputBatch),
        1_000,
    )
    .expect("throughput policy compares aggregate candidate evidence");
    assert_eq!(selection.selected_candidate_id, "laptop-shards");
    assert_ne!(selection.selected_candidate_id, "remote-gpu");
}

#[test]
fn exact_bounded_response_requirement_refuses_unproven_fast_hosts() {
    let mut control = policy(PerformanceIntent::BoundedResponse);
    control.maximum_bounded_response_us = Some(100);
    control.maximum_jitter_us = Some(5);
    let selection = select_performance_candidate(
        CheckedFormId::from("control-form"),
        &fixtures(),
        &control,
        1_000,
    )
    .expect("one candidate has exact bounded-response evidence");
    assert_eq!(selection.selected_candidate_id, "local-control");
    for rejected in selection
        .considered
        .iter()
        .filter(|item| item.candidate_id != "local-control")
    {
        assert!(matches!(
            rejected.disposition,
            PerformanceCandidateDisposition::Rejected(ref reason)
                if reason.contains("bounded-response") || reason.contains("jitter")
        ));
    }

    let no_control = fixtures()
        .into_iter()
        .filter(|item| item.candidate_id != "local-control")
        .collect::<Vec<_>>();
    assert!(select_performance_candidate(
        CheckedFormId::from("control-form"),
        &no_control,
        &control,
        1_000,
    )
    .is_err());
}

#[test]
fn hard_facets_gate_before_work_class_ranking() {
    let mut batch = policy(PerformanceIntent::ThroughputBatch);
    batch.maximum_startup_us = Some(100);
    batch.minimum_throughput_items_per_second = Some(90);
    let selection = select_performance_candidate(
        CheckedFormId::from("bounded-batch"),
        &fixtures(),
        &batch,
        1_000,
    )
    .expect("local CPU remains inside both hard facets");
    assert_eq!(selection.selected_candidate_id, "local-cpu");
    assert!(matches!(
        selection.considered[2].disposition,
        PerformanceCandidateDisposition::Rejected(ref reason) if reason.contains("startup")
    ));
}

#[test]
fn performance_class_is_never_inferred_from_candidate_or_host_names() {
    let renamed = vec![
        candidate(
            "slow-looking-name",
            &["not-fast"],
            5,
            5,
            10,
            1,
            None,
            0,
            500,
        ),
        candidate(
            "fast-looking-name",
            &["turbo"],
            50,
            50,
            1_000,
            5,
            None,
            50,
            10,
        ),
    ];
    let interactive = select_performance_candidate(
        CheckedFormId::from("name-neutral"),
        &renamed,
        &policy(PerformanceIntent::Interactive),
        1_000,
    )
    .expect("numeric reviewed profiles, not names, decide");
    let batch = select_performance_candidate(
        CheckedFormId::from("name-neutral"),
        &renamed,
        &policy(PerformanceIntent::ThroughputBatch),
        1_000,
    )
    .expect("the same Hosts have no permanent global rank");
    assert_eq!(interactive.selected_candidate_id, "slow-looking-name");
    assert_eq!(batch.selected_candidate_id, "fast-looking-name");
}

#[test]
fn stale_duplicate_or_unbounded_profiles_fail_closed() {
    assert!(select_performance_candidate(
        CheckedFormId::from("stale"),
        &fixtures(),
        &policy(PerformanceIntent::Background),
        1_101,
    )
    .is_err());

    let duplicate = vec![fixtures()[0].clone(), fixtures()[0].clone()];
    assert!(select_performance_candidate(
        CheckedFormId::from("duplicate"),
        &duplicate,
        &policy(PerformanceIntent::Background),
        1_000,
    )
    .is_err());

    let candidates = (0..=conduit_planner::MAXIMUM_PERFORMANCE_CANDIDATES)
        .map(|index| {
            candidate(
                &format!("candidate-{index}"),
                &["one-host"],
                1,
                1,
                1,
                1,
                None,
                1,
                1,
            )
        })
        .collect::<Vec<_>>();
    assert!(select_performance_candidate(
        CheckedFormId::from("bounded"),
        &candidates,
        &policy(PerformanceIntent::Background),
        1_000,
    )
    .is_err());
}
