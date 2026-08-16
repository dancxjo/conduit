use conduit_planner::{
    select_current_family_frontier, CandidateEvaluation, CandidateEvaluationDisposition,
    CandidateStructure, CurrentFamilyOffer, FactDomain, IncrementalPlanner, PlanningFact,
    PlanningFactKey, RealizationFamily, RealizationFamilyCatalog, StabilityPolicy,
};

fn key(domain: FactDomain, identity: &str) -> PlanningFactKey {
    PlanningFactKey::exact(domain, identity)
}

fn fact(domain: FactDomain, identity: &str, generation: u64) -> PlanningFact {
    PlanningFact {
        key: key(domain, identity),
        generation,
        content_identity: format!("fact/{identity}/{generation}"),
    }
}

fn family(id: &str, implementation: &str, revision: u64, prerequisite: &str) -> RealizationFamily {
    RealizationFamily {
        family_id: id.to_string(),
        semantic_contract_id: "semantic/image-filter@4".to_string(),
        implementation_contract_id: implementation.to_string(),
        implementation_contract_revision: revision,
        prerequisite_contract_ids: vec![prerequisite.to_string()],
    }
}

fn offer(
    family_id: &str,
    implementation: &str,
    revision: u64,
    prerequisite: &str,
    rank: u64,
    fact_key: PlanningFactKey,
) -> CurrentFamilyOffer {
    CurrentFamilyOffer {
        family_id: family_id.to_string(),
        semantic_contract_id: "semantic/image-filter@4".to_string(),
        implementation_contract_id: implementation.to_string(),
        implementation_contract_revision: revision,
        satisfied_prerequisite_contract_ids: vec![prerequisite.to_string()],
        policy_rank: rank,
        candidate: CandidateStructure {
            candidate_id: format!("candidate/{family_id}"),
            semantic_contract_id: "semantic/image-filter@4".to_string(),
            implementation_family_id: implementation.to_string(),
            placement_id: format!("placement/{family_id}"),
            dependencies: vec![
                key(FactDomain::Semantic, "semantic/image-filter@4"),
                fact_key,
            ],
        },
    }
}

fn evaluate(candidate: &CandidateStructure, basis: &[PlanningFact]) -> CandidateEvaluation {
    CandidateEvaluation {
        disposition: CandidateEvaluationDisposition::Admitted,
        result_identity: format!(
            "result/{}/{}",
            candidate.candidate_id,
            basis
                .iter()
                .map(|fact| fact.content_identity.as_str())
                .collect::<Vec<_>>()
                .join("+")
        ),
        total_cost: if candidate.candidate_id.ends_with("gpu") {
            10
        } else {
            100
        },
        evaluation_work_units: 20,
    }
}

#[test]
fn dominated_family_is_not_explored_but_survives_preferred_resource_loss() {
    let catalog = RealizationFamilyCatalog::new(vec![
        family("gpu", "implementation/gpu-filter@9", 9, "base/gpu@2"),
        family("cpu", "implementation/cpu-filter@3", 3, "base/cpu@1"),
    ])
    .unwrap();
    let semantic = fact(FactDomain::Semantic, "semantic/image-filter@4", 1);
    let gpu = fact(FactDomain::Resource, "resource/gpu/current", 7);
    let cpu = fact(FactDomain::Resource, "resource/cpu/current", 2);
    let gpu_offer = offer(
        "gpu",
        "implementation/gpu-filter@9",
        9,
        "base/gpu@2",
        0,
        gpu.key.clone(),
    );
    let cpu_offer = offer(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
        50,
        cpu.key.clone(),
    );
    let healthy =
        select_current_family_frontier(&catalog, &[gpu_offer.clone(), cpu_offer.clone()]).unwrap();
    assert_eq!(healthy.candidates.len(), 1);
    assert_eq!(healthy.candidates[0].candidate_id, "candidate/gpu");
    assert_eq!(healthy.metrics.dominated_candidates_not_explored, 1);

    let mut planner = IncrementalPlanner::new(2).unwrap();
    let healthy_plan = planner
        .plan(
            &healthy.candidates,
            &[semantic.clone(), gpu],
            &StabilityPolicy::disabled(),
            evaluate,
        )
        .unwrap();
    assert_eq!(healthy_plan.metrics.evaluated_candidates, 1);

    let after_gpu_loss = select_current_family_frontier(&catalog, &[cpu_offer]).unwrap();
    assert_eq!(after_gpu_loss.candidates[0].candidate_id, "candidate/cpu");
    let replacement = planner
        .plan(
            &after_gpu_loss.candidates,
            &[semantic, cpu],
            &StabilityPolicy::disabled(),
            evaluate,
        )
        .unwrap();
    assert_eq!(replacement.selected_candidate_id, "candidate/cpu");
    assert_ne!(
        replacement.selected_result_identity,
        healthy_plan.selected_result_identity
    );
    assert_eq!(catalog.families().len(), 2);
}

#[test]
fn cache_discard_cannot_erase_semantic_family_knowledge() {
    let catalog = RealizationFamilyCatalog::new(vec![family(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
    )])
    .unwrap();
    let cpu = fact(FactDomain::Resource, "resource/cpu/current", 2);
    let current = offer(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
        0,
        cpu.key.clone(),
    );
    let frontier = select_current_family_frontier(&catalog, &[current]).unwrap();
    let mut planner = IncrementalPlanner::new(1).unwrap();
    planner.discard();
    assert_eq!(
        catalog
            .family("cpu")
            .unwrap()
            .implementation_contract_revision,
        3
    );
    assert_eq!(frontier.candidates[0].candidate_id, "candidate/cpu");
}

#[test]
fn incompatible_revision_or_incomplete_prerequisites_prevent_resurrection() {
    let catalog = RealizationFamilyCatalog::new(vec![family(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
    )])
    .unwrap();
    let cpu_key = key(FactDomain::Resource, "resource/cpu/current");
    let incompatible = offer(
        "cpu",
        "implementation/cpu-filter@3",
        2,
        "base/cpu@1",
        0,
        cpu_key.clone(),
    );
    assert!(select_current_family_frontier(&catalog, &[incompatible]).is_err());

    let mut incomplete = offer(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
        0,
        cpu_key,
    );
    incomplete.satisfied_prerequisite_contract_ids.clear();
    assert!(select_current_family_frontier(&catalog, &[incomplete]).is_err());
}

#[test]
fn catalog_and_current_offer_inputs_are_finite_and_fail_closed() {
    assert!(RealizationFamilyCatalog::new(Vec::new()).is_err());
    let duplicate = family("cpu", "implementation/cpu-filter@3", 3, "base/cpu@1");
    assert!(RealizationFamilyCatalog::new(vec![duplicate.clone(), duplicate]).is_err());

    let catalog = RealizationFamilyCatalog::new(vec![family(
        "cpu",
        "implementation/cpu-filter@3",
        3,
        "base/cpu@1",
    )])
    .unwrap();
    assert!(select_current_family_frontier(&catalog, &[]).is_err());
}
