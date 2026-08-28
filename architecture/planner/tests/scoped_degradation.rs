use conduit_core::PlanId;
use conduit_planner::{
    assess_scoped_degradation, CandidateEvaluation, CandidateEvaluationDisposition,
    CandidateStructure, DegradationFragmentDisposition, DegradationInput, FactDomain,
    IncrementalPlan, IncrementalPlanner, PlanningFact, PlanningFactKey, StabilityPolicy,
};

fn key(domain: FactDomain, identity: &str) -> PlanningFactKey {
    PlanningFactKey::exact(domain, identity)
}

fn fact(domain: FactDomain, identity: &str) -> PlanningFact {
    PlanningFact {
        key: key(domain, identity),
        generation: 1,
        content_identity: format!("{identity}/generation/1"),
    }
}

fn candidate(id: &str, fragment: &str, dependencies: &[PlanningFactKey]) -> CandidateStructure {
    CandidateStructure {
        candidate_id: id.into(),
        semantic_contract_id: fragment.into(),
        implementation_family_id: format!("implementation/{id}"),
        placement_id: format!("placement/{id}"),
        dependencies: dependencies.to_vec(),
    }
}

fn evaluate(candidate: &CandidateStructure, basis: &[PlanningFact]) -> CandidateEvaluation {
    let lost = basis.iter().find(|fact| fact.generation > 1);
    CandidateEvaluation {
        disposition: lost.map_or(CandidateEvaluationDisposition::Admitted, |fact| {
            CandidateEvaluationDisposition::Rejected(format!(
                "exact {:?} {} generation {} is unavailable",
                fact.key.domain, fact.key.identity, fact.generation
            ))
        }),
        result_identity: format!(
            "result/{}/{}",
            candidate.candidate_id,
            basis
                .iter()
                .map(|fact| fact.generation.to_string())
                .collect::<Vec<_>>()
                .join("-")
        ),
        total_cost: if candidate.candidate_id.contains("primary") {
            10
        } else {
            20
        },
        evaluation_work_units: 5,
    }
}

struct FragmentFixture {
    fragment_id: &'static str,
    previous_candidate_id: &'static str,
    candidates: Vec<CandidateStructure>,
    planner: IncrementalPlanner,
}

impl FragmentFixture {
    fn new(
        fragment_id: &'static str,
        previous_candidate_id: &'static str,
        candidates: Vec<CandidateStructure>,
        facts: &[PlanningFact],
    ) -> Self {
        let mut planner = IncrementalPlanner::new(8).unwrap();
        let initial = planner
            .plan(&candidates, facts, &StabilityPolicy::disabled(), evaluate)
            .unwrap();
        assert_eq!(initial.selected_candidate_id, previous_candidate_id);
        Self {
            fragment_id,
            previous_candidate_id,
            candidates,
            planner,
        }
    }

    fn replan(&mut self, facts: &[PlanningFact]) -> IncrementalPlan {
        self.planner
            .plan(
                &self.candidates,
                facts,
                &StabilityPolicy::disabled(),
                evaluate,
            )
            .unwrap()
    }

    fn input(&self, fresh_plan: IncrementalPlan) -> DegradationInput {
        DegradationInput {
            fragment_id: self.fragment_id.into(),
            previous_candidate_id: self.previous_candidate_id.into(),
            candidates: self.candidates.clone(),
            fresh_plan: Some(fresh_plan),
            refusal: None,
        }
    }
}

fn facts() -> Vec<PlanningFact> {
    vec![
        fact(FactDomain::Host, "host/edge"),
        fact(FactDomain::Resource, "resource/edge-input"),
        fact(FactDomain::Host, "host/compute"),
        fact(FactDomain::Resource, "resource/compute-gpu"),
        fact(FactDomain::Host, "host/browser"),
        fact(FactDomain::Host, "host/spare"),
        fact(FactDomain::Resource, "resource/spare-cpu"),
        fact(FactDomain::Line, "line/edge-compute"),
        fact(FactDomain::Line, "line/edge-spare"),
    ]
}

fn fixtures(facts: &[PlanningFact]) -> Vec<FragmentFixture> {
    vec![
        FragmentFixture::new(
            "fragment/edge-input-filter",
            "edge-primary",
            vec![candidate(
                "edge-primary",
                "fragment/edge-input-filter",
                &[
                    key(FactDomain::Host, "host/edge"),
                    key(FactDomain::Resource, "resource/edge-input"),
                ],
            )],
            facts,
        ),
        FragmentFixture::new(
            "fragment/heavy-compute",
            "compute-primary",
            vec![
                candidate(
                    "compute-primary",
                    "fragment/heavy-compute",
                    &[
                        key(FactDomain::Host, "host/compute"),
                        key(FactDomain::Resource, "resource/compute-gpu"),
                    ],
                ),
                candidate(
                    "compute-fallback",
                    "fragment/heavy-compute",
                    &[
                        key(FactDomain::Host, "host/spare"),
                        key(FactDomain::Resource, "resource/spare-cpu"),
                    ],
                ),
            ],
            facts,
        ),
        FragmentFixture::new(
            "fragment/transport",
            "line-primary",
            vec![
                candidate(
                    "line-primary",
                    "fragment/transport",
                    &[key(FactDomain::Line, "line/edge-compute")],
                ),
                candidate(
                    "line-fallback",
                    "fragment/transport",
                    &[key(FactDomain::Line, "line/edge-spare")],
                ),
            ],
            facts,
        ),
        FragmentFixture::new(
            "fragment/browser-presentation",
            "browser-primary",
            vec![candidate(
                "browser-primary",
                "fragment/browser-presentation",
                &[key(FactDomain::Host, "host/browser")],
            )],
            facts,
        ),
    ]
}

fn lose(facts: &mut [PlanningFact], changed: &PlanningFactKey) {
    let fact = facts.iter_mut().find(|fact| fact.key == *changed).unwrap();
    fact.generation += 1;
    fact.content_identity = format!("{}/generation/{}", fact.key.identity, fact.generation);
}

fn replacement_assessment(changed: PlanningFactKey, expected_fragment: &str) {
    let mut current = facts();
    let mut fixtures = fixtures(&current);
    lose(&mut current, &changed);
    let inputs = fixtures
        .iter_mut()
        .map(|fixture| {
            let fresh = fixture.replan(&current);
            fixture.input(fresh)
        })
        .collect::<Vec<_>>();
    let historical = PlanId::from("plan/heterogeneous/old");
    let immutable = historical.clone();
    let assessment = assess_scoped_degradation(
        historical.clone(),
        Some(PlanId::from(format!(
            "plan/heterogeneous/replacement/{}",
            changed.identity
        ))),
        &[changed],
        &inputs,
    )
    .unwrap();

    assert_eq!(historical, immutable);
    assert_ne!(
        assessment.replacement_plan_id.as_ref(),
        Some(&assessment.previous_plan_id)
    );
    assert_eq!(assessment.automatic_retry_count, 0);
    assert_eq!(assessment.what_failed().len(), 1);
    assert_eq!(assessment.what_failed()[0].fragment_id, expected_fragment);
    assert!(matches!(
        assessment.what_failed()[0].disposition,
        DegradationFragmentDisposition::Replaced { .. }
    ));
    assert_eq!(assessment.what_still_works().len(), 3);
    assert!(assessment
        .what_still_works()
        .iter()
        .all(|fragment| fragment.reused_unaffected_structure));
}

#[test]
fn gpu_compute_host_and_line_loss_each_replace_only_the_dependent_fragment() {
    replacement_assessment(
        key(FactDomain::Resource, "resource/compute-gpu"),
        "fragment/heavy-compute",
    );
    replacement_assessment(
        key(FactDomain::Host, "host/compute"),
        "fragment/heavy-compute",
    );
    replacement_assessment(
        key(FactDomain::Line, "line/edge-compute"),
        "fragment/transport",
    );
}

#[test]
fn browser_loss_preserves_three_fragments_and_reports_a_specific_refusal() {
    let changed = key(FactDomain::Host, "host/browser");
    let mut current = facts();
    let mut fixtures = fixtures(&current);
    lose(&mut current, &changed);
    let mut inputs = Vec::new();
    for fixture in &mut fixtures {
        if fixture.fragment_id == "fragment/browser-presentation" {
            let error = fixture
                .planner
                .plan(
                    &fixture.candidates,
                    &current,
                    &StabilityPolicy::disabled(),
                    evaluate,
                )
                .expect_err("lost only browser candidate refuses fresh planning");
            inputs.push(DegradationInput {
                fragment_id: fixture.fragment_id.into(),
                previous_candidate_id: fixture.previous_candidate_id.into(),
                candidates: fixture.candidates.clone(),
                fresh_plan: None,
                refusal: Some(format!("browser presentation unavailable: {error}")),
            });
        } else {
            let fresh = fixture.replan(&current);
            inputs.push(fixture.input(fresh));
        }
    }
    let assessment = assess_scoped_degradation(
        PlanId::from("plan/heterogeneous/old"),
        None,
        &[changed],
        &inputs,
    )
    .unwrap();
    assert!(assessment.replacement_plan_id.is_none());
    assert_eq!(assessment.what_still_works().len(), 3);
    assert!(matches!(
        &assessment.what_failed()[0].disposition,
        DegradationFragmentDisposition::Refused { reason }
            if reason.contains("browser presentation unavailable")
    ));
}

#[test]
fn unrelated_fourth_host_change_reuses_every_existing_fragment() {
    let mut current = facts();
    let mut fixtures = fixtures(&current);
    let unrelated = key(FactDomain::Host, "host/spare");
    lose(&mut current, &unrelated);
    for fixture in &mut fixtures {
        if fixture.fragment_id == "fragment/heavy-compute" {
            // The dormant fallback is reevaluated, but the selected active compute
            // realization remains exact and reusable.
            let fresh = fixture.replan(&current);
            assert_eq!(fresh.selected_candidate_id, "compute-primary");
            assert!(
                fresh
                    .considered
                    .iter()
                    .find(|candidate| candidate.candidate_id == "compute-primary")
                    .unwrap()
                    .reused
            );
        } else {
            let fresh = fixture.replan(&current);
            assert_eq!(fresh.metrics.invalidated_candidates, 0);
            assert_eq!(
                fresh.metrics.reused_candidates,
                u32::try_from(fixture.candidates.len()).unwrap()
            );
        }
    }
}

#[test]
fn stale_reuse_mutable_plan_repair_and_generic_refusal_fail_closed() {
    let changed = key(FactDomain::Resource, "resource/compute-gpu");
    let current = facts();
    let mut fixtures = fixtures(&current);
    let inputs = fixtures
        .iter_mut()
        .map(|fixture| {
            let fresh = fixture.replan(&current);
            fixture.input(fresh)
        })
        .collect::<Vec<_>>();
    assert!(assess_scoped_degradation(
        PlanId::from("plan/old"),
        Some(PlanId::from("plan/old")),
        std::slice::from_ref(&changed),
        &inputs,
    )
    .is_err());

    let browser = key(FactDomain::Host, "host/browser");
    let refusal = DegradationInput {
        fragment_id: "fragment/browser-presentation".into(),
        previous_candidate_id: "browser-primary".into(),
        candidates: fixtures[3].candidates.clone(),
        fresh_plan: None,
        refusal: Some(String::new()),
    };
    assert!(
        assess_scoped_degradation(PlanId::from("plan/old"), None, &[browser], &[refusal],).is_err()
    );
}
