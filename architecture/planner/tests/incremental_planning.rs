use conduit_planner::{
    plan_cold, CandidateEvaluation, CandidateEvaluationDisposition, CandidateStructure, FactDomain,
    IncrementalPlanner, PlanningFact, PlanningFactKey, StabilityPolicy,
};

fn key(domain: FactDomain, identity: &str) -> PlanningFactKey {
    PlanningFactKey::exact(domain, identity)
}

fn fact(domain: FactDomain, identity: &str, generation: u64) -> PlanningFact {
    PlanningFact {
        key: key(domain, identity),
        generation,
        content_identity: format!("fact-content/{identity}/generation/{generation}"),
    }
}

fn candidate(
    id: &str,
    placement: &str,
    extra_dependencies: &[(FactDomain, &str)],
) -> CandidateStructure {
    let mut dependencies = vec![
        key(FactDomain::Semantic, "checked-form/text-pipeline@sha256:01"),
        key(FactDomain::Implementation, "family/text-upper@1"),
        key(FactDomain::Policy, "policy/efficiency@7"),
    ];
    dependencies.extend(
        extra_dependencies
            .iter()
            .map(|(domain, identity)| key(*domain, identity)),
    );
    CandidateStructure {
        candidate_id: id.to_string(),
        semantic_contract_id: "checked-form/text-pipeline@sha256:01".to_string(),
        implementation_family_id: "family/text-upper@1".to_string(),
        placement_id: placement.to_string(),
        dependencies,
    }
}

fn base_facts() -> Vec<PlanningFact> {
    vec![
        fact(
            FactDomain::Semantic,
            "checked-form/text-pipeline@sha256:01",
            1,
        ),
        fact(FactDomain::Implementation, "family/text-upper@1", 1),
        fact(FactDomain::Policy, "policy/efficiency@7", 7),
        fact(FactDomain::Host, "host/local@sha256:10", 1),
        fact(FactDomain::Boot, "boot/local@sha256:11", 1),
        fact(FactDomain::Offer, "offer/local@sha256:12", 1),
        fact(FactDomain::Resource, "resource/local-cpu@sha256:13", 1),
        fact(FactDomain::Authority, "authority/local@sha256:14", 1),
        fact(FactDomain::Host, "host/remote@sha256:20", 1),
        fact(FactDomain::Boot, "boot/remote@sha256:21", 1),
        fact(FactDomain::Offer, "offer/remote@sha256:22", 1),
        fact(FactDomain::Resource, "resource/remote-cpu@sha256:23", 1),
        fact(FactDomain::Authority, "authority/remote@sha256:24", 1),
        fact(FactDomain::Line, "line/local-remote@sha256:25", 1),
    ]
}

fn local() -> CandidateStructure {
    candidate(
        "local",
        "placement/local@sha256:30",
        &[
            (FactDomain::Host, "host/local@sha256:10"),
            (FactDomain::Boot, "boot/local@sha256:11"),
            (FactDomain::Offer, "offer/local@sha256:12"),
            (FactDomain::Resource, "resource/local-cpu@sha256:13"),
            (FactDomain::Authority, "authority/local@sha256:14"),
        ],
    )
}

fn remote() -> CandidateStructure {
    candidate(
        "remote",
        "placement/remote@sha256:31",
        &[
            (FactDomain::Host, "host/remote@sha256:20"),
            (FactDomain::Boot, "boot/remote@sha256:21"),
            (FactDomain::Offer, "offer/remote@sha256:22"),
            (FactDomain::Resource, "resource/remote-cpu@sha256:23"),
            (FactDomain::Authority, "authority/remote@sha256:24"),
            (FactDomain::Line, "line/local-remote@sha256:25"),
        ],
    )
}

fn evaluate(candidate: &CandidateStructure, basis: &[PlanningFact]) -> CandidateEvaluation {
    let changed_hard_fact = basis.iter().find(|fact| {
        matches!(
            fact.key.domain,
            FactDomain::Boot
                | FactDomain::Offer
                | FactDomain::Resource
                | FactDomain::Authority
                | FactDomain::Line
        ) && fact.generation > 1
    });
    let disposition = match changed_hard_fact {
        Some(fact) => CandidateEvaluationDisposition::Rejected(format!(
            "fresh-{:?}-generation-{}-refused",
            fact.key.domain, fact.generation
        )),
        None => CandidateEvaluationDisposition::Admitted,
    };
    let base_cost = match candidate.candidate_id.as_str() {
        "local" => 100,
        "remote" => 80,
        "new-host" => 70,
        "steady" => 100,
        "cheapest" => 95,
        _ => 120,
    };
    let generations = basis
        .iter()
        .map(|fact| fact.generation.to_string())
        .collect::<Vec<_>>()
        .join("-");
    CandidateEvaluation {
        disposition,
        result_identity: format!("result/{}/basis/{generations}", candidate.candidate_id),
        total_cost: base_cost,
        evaluation_work_units: 25,
    }
}

#[test]
fn localized_line_change_reuses_structure_and_matches_cold_planning() {
    let candidates = vec![local(), remote()];
    let mut facts = base_facts();
    let mut planner = IncrementalPlanner::new(4).unwrap();
    let first = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(first.selected_candidate_id, "remote");
    assert_eq!(first.metrics.evaluated_candidates, 2);
    assert_eq!(first.metrics.reused_candidates, 0);

    facts
        .iter_mut()
        .find(|fact| fact.key.domain == FactDomain::Line)
        .unwrap()
        .generation = 2;
    let incremental = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    let cold = plan_cold(&candidates, &facts, &StabilityPolicy::disabled(), evaluate).unwrap();

    assert_eq!(incremental.selected_candidate_id, "local");
    assert_eq!(
        incremental.selected_candidate_id,
        cold.selected_candidate_id
    );
    assert_eq!(
        incremental.selected_result_identity,
        cold.selected_result_identity
    );
    assert_eq!(incremental.selected_cost, cold.selected_cost);
    assert_eq!(incremental.metrics.reused_candidates, 1);
    assert_eq!(incremental.metrics.invalidated_candidates, 1);
    assert_eq!(incremental.metrics.evaluated_candidates, 1);
    assert!(
        incremental.metrics.logical_latency_work_units < cold.metrics.logical_latency_work_units
    );
    assert!(incremental.considered[0].reused);
    assert!(matches!(
        incremental.considered[1].evaluation.disposition,
        CandidateEvaluationDisposition::Rejected(ref reason)
            if reason.contains("Line")
    ));
}

#[test]
fn unrelated_presentation_change_and_new_host_preserve_existing_work() {
    let mut candidates = vec![local(), remote()];
    let mut facts = base_facts();
    facts.push(fact(
        FactDomain::Semantic,
        "presentation/viewport@sha256:99",
        1,
    ));
    let mut planner = IncrementalPlanner::new(4).unwrap();
    planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();

    facts.last_mut().unwrap().generation = 2;
    let presentation_only = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(presentation_only.metrics.reused_candidates, 2);
    assert_eq!(presentation_only.metrics.invalidated_candidates, 0);
    assert_eq!(presentation_only.metrics.evaluation_work_units, 0);

    facts.extend([
        fact(FactDomain::Host, "host/new@sha256:40", 1),
        fact(FactDomain::Boot, "boot/new@sha256:41", 1),
        fact(FactDomain::Offer, "offer/new@sha256:42", 1),
    ]);
    candidates.push(candidate(
        "new-host",
        "placement/new@sha256:43",
        &[
            (FactDomain::Host, "host/new@sha256:40"),
            (FactDomain::Boot, "boot/new@sha256:41"),
            (FactDomain::Offer, "offer/new@sha256:42"),
        ],
    ));
    let expanded = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(expanded.metrics.reused_candidates, 2);
    assert_eq!(expanded.metrics.evaluated_candidates, 1);
    assert_eq!(expanded.selected_candidate_id, "new-host");
}

#[test]
fn boot_resource_and_authority_generations_invalidate_immediately() {
    for domain in [
        FactDomain::Boot,
        FactDomain::Resource,
        FactDomain::Authority,
    ] {
        let candidates = vec![local(), remote()];
        let mut facts = base_facts();
        let mut planner = IncrementalPlanner::new(4).unwrap();
        planner
            .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
            .unwrap();
        facts
            .iter_mut()
            .find(|fact| fact.key.domain == domain && fact.key.identity.contains("local"))
            .unwrap()
            .generation = 2;
        let replanned = planner
            .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
            .unwrap();
        let local = replanned
            .considered
            .iter()
            .find(|candidate| candidate.candidate_id == "local")
            .unwrap();
        assert!(!local.reused);
        assert!(matches!(
            local.evaluation.disposition,
            CandidateEvaluationDisposition::Rejected(_)
        ));
        assert_eq!(replanned.metrics.invalidated_candidates, 1);
        assert_eq!(replanned.selected_candidate_id, "remote");
    }
}

#[test]
fn stable_locality_is_bounded_and_never_overrides_hard_truth() {
    let steady = candidate(
        "steady",
        "placement/steady@sha256:50",
        &[(FactDomain::Resource, "resource/local-cpu@sha256:13")],
    );
    let cheapest = candidate(
        "cheapest",
        "placement/cheapest@sha256:51",
        &[(FactDomain::Resource, "resource/remote-cpu@sha256:23")],
    );
    let candidates = vec![steady, cheapest];
    let mut facts = base_facts();
    let stability = StabilityPolicy {
        previous_placement_id: Some("placement/steady@sha256:50".to_string()),
        maximum_cost_penalty: 10,
    };
    let mut planner = IncrementalPlanner::new(4).unwrap();
    let stable = planner
        .plan(&candidates, &facts, &stability, evaluate)
        .unwrap();
    assert_eq!(stable.selected_candidate_id, "steady");
    assert!(stable.stability_preference_applied);

    facts
        .iter_mut()
        .find(|fact| fact.key.identity == "resource/local-cpu@sha256:13")
        .unwrap()
        .generation = 2;
    let lost = planner
        .plan(&candidates, &facts, &stability, evaluate)
        .unwrap();
    assert_eq!(lost.selected_candidate_id, "cheapest");
    assert!(!lost.stability_preference_applied);
}

#[test]
fn cache_is_finite_inspectable_evictable_and_safe_to_discard() {
    let candidates = vec![
        local(),
        remote(),
        candidate(
            "third",
            "placement/third@sha256:60",
            &[(FactDomain::Host, "host/local@sha256:10")],
        ),
    ];
    let facts = base_facts();
    let mut planner = IncrementalPlanner::new(2).unwrap();
    let first = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(first.metrics.cache_entries, 2);
    assert_eq!(first.metrics.cache_capacity, 2);
    assert_eq!(first.metrics.discarded_cache_entries, 1);
    assert_eq!(planner.retained_candidate_ids(), vec!["remote", "third"]);

    planner.discard();
    assert!(planner.retained_candidate_ids().is_empty());
    let after_discard = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(after_discard.metrics.reused_candidates, 0);
    assert_eq!(after_discard.metrics.evaluated_candidates, 3);
    assert_eq!(
        after_discard.selected_candidate_id,
        first.selected_candidate_id
    );
}

#[test]
fn malformed_unbounded_or_ambient_dependency_inputs_fail_closed() {
    assert!(IncrementalPlanner::new(0).is_err());
    assert!(IncrementalPlanner::new(conduit_planner::MAXIMUM_CACHED_CANDIDATES + 1).is_err());

    let mut missing = local();
    missing.dependencies.push(key(
        FactDomain::Host,
        "host/not-in-current-truth@sha256:dead",
    ));
    let mut planner = IncrementalPlanner::new(2).unwrap();
    assert!(planner
        .plan(
            &[missing],
            &base_facts(),
            &StabilityPolicy::disabled(),
            evaluate,
        )
        .is_err());

    let mut duplicate = base_facts();
    duplicate.push(duplicate[0].clone());
    assert!(planner
        .plan(
            &[local()],
            &duplicate,
            &StabilityPolicy::disabled(),
            evaluate,
        )
        .is_err());
}

#[test]
fn exact_content_change_invalidates_even_if_a_producer_reuses_its_generation() {
    let candidates = vec![local(), remote()];
    let mut facts = base_facts();
    let mut planner = IncrementalPlanner::new(4).unwrap();
    planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();

    facts
        .iter_mut()
        .find(|fact| fact.key.identity == "authority/local@sha256:14")
        .unwrap()
        .content_identity = "fact-content/authority/local/changed".to_string();
    let replanned = planner
        .plan(&candidates, &facts, &StabilityPolicy::disabled(), evaluate)
        .unwrap();
    assert_eq!(replanned.metrics.invalidated_candidates, 1);
    assert_eq!(replanned.metrics.reused_candidates, 1);
}
