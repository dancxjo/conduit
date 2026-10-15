//! Cache/cold equivalence for actual ordinary Plans and modeled cost-source identities.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SOURCE: &str =
    "plot cached {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n";
#[derive(Clone)]
struct Facts {
    milliseconds: [i128; 2],
    capacity: [u32; 2],
    denied: [bool; 2],
}
impl Default for Facts {
    fn default() -> Self {
        Self {
            milliseconds: [10, 1],
            capacity: [4, 4],
            denied: [false; 2],
        }
    }
}
fn plot() -> conduit_plot::CheckedPlot {
    conduit_plot::parse_with_startup(
        SOURCE,
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap()
}
fn key(domain: FactDomain, index: usize) -> PlanningFactKey {
    PlanningFactKey::exact(domain, format!("real-plan/host/{index}"))
}
fn host(f: &Facts, i: usize) -> HostAdvertisement {
    let mut host = common::competing_hosts()[i].clone();
    if f.denied[i] {
        let offer = &mut host.capabilities[0];
        offer.authority_requirements.push(AuthorityRequirement {
            contract_id: "cache/authority@1".into(),
            host_call_contract_id: WAIT_HOST_CALL_CONTRACT.into(),
            subject_kind: offer.kind_id.clone(),
        });
    }
    host
}
fn dependency_keys(i: usize) -> [PlanningFactKey; 4] {
    [
        key(FactDomain::Offer, i),
        key(FactDomain::Resource, i),
        key(FactDomain::Authority, i),
        PlanningFactKey::exact(FactDomain::Resource, format!("cost/host/{i}")),
    ]
}
fn snapshots(f: &Facts) -> Vec<PlanningFact> {
    let hosts = [host(f, 0), host(f, 1)];
    let mut facts = Vec::new();
    for (i, host) in hosts.iter().enumerate() {
        let contents = [
            serde_json::to_string(host).unwrap(),
            f.capacity[i].to_string(),
            f.denied[i].to_string(),
            f.milliseconds[i].to_string(),
        ];
        for (key, content) in dependency_keys(i).into_iter().zip(contents) {
            facts.push(PlanningFact {
                key,
                generation: 1,
                content_identity: format!("{:x}", Sha256::digest(content.as_bytes())),
            });
        }
    }
    facts
}
fn candidates() -> Vec<CandidateStructure> {
    (0..2)
        .map(|i| CandidateStructure {
            candidate_id: i.to_string(),
            semantic_contract_id: plot().checked_plot_id.as_str().to_owned(),
            implementation_family_id: "flow/pulse@1".into(),
            placement_id: format!("host/{i}"),
            dependencies: dependency_keys(i).into(),
        })
        .collect()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |value| {
        let quantity = RationalQuantity::new(
            ExactRational::from_i128(value),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let duration = CostDuration::admit(quantity, &mut work).unwrap();
        CostDurationRange::admit(duration, duration, &mut work).unwrap()
    };
    let stages = CostForecastStages {
        setup: Some(point(0)),
        queue: Some(point(0)),
        service: Some(point(ms)),
        transfer: Some(point(0)),
    };
    FulfillmentCostForecast::sequential(stages, &mut work).unwrap()
}

fn ordinary(facts: &Facts, i: usize) -> Result<Plan, String> {
    let plot = plot();
    let hosts = [host(facts, i)];
    let host = &hosts[0];
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "cache/clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let window = CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(200, clock.clone()).unwrap(),
    )
    .unwrap();
    let now = MonotonicInstant::new(101, clock).unwrap();
    let profile: ArtifactId = "cache/calibration@1".into();
    let calibration = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let contexts = [CostEvaluationContext {
        host,
        now: &now,
        calibrations: &calibration,
        execution_observations: &[],
    }];
    let sign: SignId = format!("modeled/cost/{i}/{}", facts.milliseconds[i]).into();
    let source = CostEvidenceSource::bind(
        host,
        &host.capabilities[0],
        None,
        &sign,
        calibration[0],
        window.clone(),
    )
    .unwrap();
    let demand = CostDemand::admit(&plot.gears[0], &[]).unwrap();
    let prediction = forecast(facts.milliseconds[i]);
    let forecasts = [CostedForecast::bind(&source, demand, &prediction).unwrap()];
    let mut requirements = HardRealizationRequirements::default();
    if facts.denied[i] {
        requirements.permitted_authority_contracts = Some(BTreeSet::new());
    }
    let policy = RealizationPolicy::default();
    let operations = [CostPlanningOperation {
        demand,
        forecasts: &forecasts,
        requirements: &requirements,
        tie_policy: &policy,
        intent: PerformanceIntent::Interactive,
        unavailable: UnavailableCostPolicy::RequireKnown,
    }];
    let mut observations = common::resource_observations(&hosts);
    for observation in &mut observations {
        observation.unreserved_units = facts.capacity[i];
    }
    let timed: Vec<_> = observations
        .iter()
        .map(|o| TimedResourceObservation::admit(o, window.clone()).unwrap())
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    plan_selected_realizations_with_costs(
        &plot,
        CostPlanningInputs {
            hosts: &hosts,
            contexts: &contexts,
            operations: &operations,
            advertisements: &[],
            observations: &timed,
            bases: &[],
            options: PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .map(|result| {
        assert!(verify_plan(&result.plan));
        result.plan
    })
    .map_err(|e| format!("{e:?}"))
}
fn evaluate(
    facts: &Facts,
    candidate: &CandidateStructure,
    basis: &[PlanningFact],
    retained: &mut BTreeMap<String, Plan>,
) -> CandidateEvaluation {
    let i: usize = candidate.candidate_id.parse().unwrap();
    let expected = snapshots(facts);
    for fact in basis {
        assert!(expected.contains(fact));
    }
    let (disposition, result_identity) = match ordinary(facts, i) {
        Ok(plan) => {
            let id = plan.plan_id.as_str().to_owned();
            if let Some(previous) = retained.insert(id.clone(), plan.clone()) {
                assert_eq!(previous, plan);
            }
            (CandidateEvaluationDisposition::Admitted, id)
        }
        Err(reason) => (
            CandidateEvaluationDisposition::Rejected(reason.clone()),
            reason,
        ),
    };
    CandidateEvaluation {
        disposition,
        result_identity,
        total_cost: u64::try_from(facts.milliseconds[i]).unwrap(),
        evaluation_work_units: 1,
    }
}
fn compare(
    planner: &mut IncrementalPlanner,
    facts: &Facts,
    retained: &mut BTreeMap<String, Plan>,
) -> Result<IncrementalPlan, PlannerError> {
    let candidates = candidates();
    let basis = snapshots(facts);
    let policy = StabilityPolicy::disabled();
    let incremental = planner.plan(&candidates, &basis, &policy, |c, b| {
        evaluate(facts, c, b, retained)
    });
    let cold = plan_cold(&candidates, &basis, &policy, |c, b| {
        evaluate(facts, c, b, retained)
    });
    match (&incremental, &cold) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.selected_result_identity, b.selected_result_identity);
            assert_eq!(a.selected_cost, b.selected_cost);
            for (x, y) in a.considered.iter().zip(&b.considered) {
                assert_eq!(x.evaluation, y.evaluation);
            }
            assert!(verify_plan(&retained[&a.selected_result_identity]));
        }
        (Err(a), Err(b)) => assert_eq!(format!("{a:?}"), format!("{b:?}")),
        _ => panic!("incremental/cold hard admissibility differs"),
    }
    incremental
}
#[test]
fn changed_cost_and_resource_facts_reuse_only_unaffected_real_plans() {
    let mut facts = Facts::default();
    let mut planner = IncrementalPlanner::new(2).unwrap();
    let mut retained = BTreeMap::new();
    let first = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(first.selected_candidate_id, "1");
    let original = retained[&first.selected_result_identity].clone();
    let unchanged = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(unchanged.metrics.reused_candidates, 2);
    facts.milliseconds[1] = 20;
    let cost = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(cost.selected_candidate_id, "0");
    assert_eq!(cost.metrics.invalidated_candidates, 1);
    assert_eq!(cost.metrics.reused_candidates, 1);
    assert_ne!(
        cost.selected_result_identity,
        first.selected_result_identity
    );
    facts.capacity[0] = 0;
    let resource = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(resource.selected_candidate_id, "1");
    assert_eq!(resource.metrics.invalidated_candidates, 1);
    assert_eq!(resource.metrics.reused_candidates, 1);
    assert!(matches!(
        resource.considered[0].evaluation.disposition,
        CandidateEvaluationDisposition::Rejected(_)
    ));
    assert_eq!(retained[&first.selected_result_identity], original);
    assert!(verify_plan(&original));
    facts.capacity[1] = 0;
    assert!(compare(&mut planner, &facts, &mut retained).is_err());
    facts.capacity = [4, 4];
    assert!(compare(&mut planner, &facts, &mut retained).is_ok());
}

#[test]
fn authority_content_changes_invalidate_even_at_the_same_generation() {
    let mut facts = Facts::default();
    let mut planner = IncrementalPlanner::new(2).unwrap();
    let mut retained = BTreeMap::new();
    let first = compare(&mut planner, &facts, &mut retained).unwrap();
    let old = retained[&first.selected_result_identity].clone();
    facts.denied[1] = true;
    let next = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(next.selected_candidate_id, "0");
    assert_ne!(
        first.selected_result_identity,
        next.selected_result_identity
    );
    assert_eq!(next.metrics.reused_candidates, 1);
    assert_eq!(next.metrics.invalidated_candidates, 1);
    let CandidateEvaluationDisposition::Rejected(reason) =
        &next.considered[1].evaluation.disposition
    else {
        panic!("unauthorized candidate reused")
    };
    assert!(reason.contains("AuthorityContractAllowlist"), "{reason}");
    assert_eq!(retained[&first.selected_result_identity], old);
    assert!(verify_plan(&old));
    facts.denied[0] = true;
    assert!(compare(&mut planner, &facts, &mut retained).is_err());
    facts.denied = [false; 2];
    let restored = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(
        restored.selected_result_identity,
        first.selected_result_identity
    );
}
