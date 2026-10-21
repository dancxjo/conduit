//! Fixed-clock conformance with real reserved/published local providers.
//! Forecast intervals here are declared fixtures, not measured speedups.
#[path = "../../examples/resource_replacement_cost/fixture.rs"]
#[allow(dead_code)]
mod fixture;
use conduit_core::*;
use conduit_planner::resource_cost::*;
pub(super) use fixture::Cases;

pub(super) fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
pub(super) fn duration(ns: u64) -> CostDuration {
    CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(ns.into(), 1_000_000_000, &mut work()).unwrap(),
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work(),
    )
    .unwrap()
}

pub(super) fn policy() -> ReluctantReplanPolicy {
    ReluctantReplanPolicy::admit(
        3,
        duration(100),
        duration(10_000),
        duration(250),
        duration(1_000),
        duration(10),
        1,
        &mut work(),
    )
    .unwrap()
}
fn clock(case: &Cases) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        case.host.host_id.clone(),
        case.host.boot_id.clone(),
        "fixture/replan-clock".into(),
        TemporalScale::Nanoseconds,
        1,
        0,
    )
    .unwrap()
}
pub(super) fn instant(case: &Cases, tick: u64) -> MonotonicInstant {
    MonotonicInstant::new(tick, clock(case)).unwrap()
}
fn source<'a>(
    case: &'a Cases,
    gear: &PlannedGear,
    sign: &'a SignId,
    revision: CostCalibrationRevision<'a>,
    tick: u64,
) -> CostEvidenceSource<'a> {
    CostEvidenceSource::bind(
        &case.host,
        case.host
            .capabilities
            .iter()
            .find(|c| c.capability_id == gear.capability_id)
            .unwrap(),
        Some(
            case.host
                .resources
                .iter()
                .find(|r| r.pool_id == gear.resources[0].pool_id)
                .unwrap(),
        ),
        sign,
        revision,
        CostObservationWindow::admit(instant(case, tick), instant(case, 10_000)).unwrap(),
    )
    .unwrap()
}
pub(super) fn with_facts(run: impl FnOnce(&Cases, &ReplacementCostFacts<'_>)) {
    let case = Cases::new().unwrap();
    let old = &case.previous.fragments[0].placements[0];
    let new = &case.candidate.fragments[0].placements[0];
    let mut owner = ResourceAdmissionOwner::new(case.host.clone());
    let observations = case.observations();
    for (plan, gear) in [(&case.previous, old), (&case.candidate, new)] {
        owner
            .admit_planned_placement(plan.plan_id.clone(), gear, &observations)
            .unwrap();
    }
    let reader: AuthorityGrantId = "fixture/replan-reader".into();
    let payload = fixture::payload();
    let origin = fixture::provider(&old.resources[0], &reader, None, None)
        .initialize(&payload)
        .unwrap();
    let target = fixture::provider_with_handle(
        &new.resources[0],
        &reader,
        None,
        None,
        "fixture/new-frame".into(),
    )
    .initialize(&payload)
    .unwrap();
    let identity: ArtifactId = "fixture/replan-costs".into();
    let revision = CostCalibrationRevision::new(&identity, 1).unwrap();
    let signs: Vec<SignId> = (0..5)
        .map(|i| format!("fixture/stage/{i}").into())
        .collect();
    let measurements: Vec<_> = (0..5)
        .map(|i| {
            CostMeasuredDuration::measure(
                &instant(&case, 100 + i * 10),
                &instant(&case, 101 + i * 10),
                &mut work(),
            )
            .unwrap()
        })
        .collect();
    let sources: Vec<_> = (0..5)
        .map(|i| {
            source(
                &case,
                if i == 4 { old } else { new },
                &signs[i],
                revision,
                measurements[i].completed().ticks(),
            )
        })
        .collect();
    let stages = [
        ReplacementCostStage::Preparation,
        ReplacementCostStage::Transfer,
        ReplacementCostStage::Residency,
        ReplacementCostStage::Migration,
        ReplacementCostStage::Continuity,
    ];
    let parts: Vec<_> = (0..5)
        .map(|i| {
            ReplacementCostComponent::bind(
                stages[i],
                &sources[i],
                if i == 4 {
                    &case.previous
                } else {
                    &case.candidate
                },
                &measurements[i],
                Some(if i == 4 {
                    origin.reference()
                } else {
                    target.reference()
                }),
            )
            .unwrap()
        })
        .collect();
    let old_residence =
        CostResidentResource::bind(&sources[4], &case.previous, old, &owner, &origin).unwrap();
    let new_residence =
        CostResidentResource::bind(&sources[2], &case.candidate, new, &owner, &target).unwrap();
    let residences = [&old_residence, &new_residence];
    let calibrations = [revision];
    let now = instant(&case, 300);
    let context = CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let facts = ReplacementCostFacts::admit(
        &case.previous,
        old,
        &case.candidate,
        new,
        core::array::from_fn(|i| Some(&parts[i])),
        &residences,
        &[context],
        &mut work(),
    )
    .unwrap();
    run(&case, &facts);
}
pub(super) fn evaluate_with_budget(
    gate: &mut ReluctantReplanGate,
    case: &Cases,
    facts: &ReplacementCostFacts<'_>,
    ticks: &[u64],
    old_ns: &[u64],
    now: u64,
    budget: u64,
) -> Result<ReluctantReplanDecision, ReluctantReplanRefusal> {
    let old = &case.previous.fragments[0].placements[0];
    let new = &case.candidate.fragments[0].placements[0];
    let identity: ArtifactId = "fixture/replan-costs".into();
    let revision = CostCalibrationRevision::new(&identity, 1).unwrap();
    let signs: Vec<SignId> = ticks
        .iter()
        .flat_map(|tick| {
            [
                format!("fixture/old/{tick}").into(),
                format!("fixture/new/{tick}").into(),
            ]
        })
        .collect();
    let sources: Vec<_> = ticks
        .iter()
        .enumerate()
        .flat_map(|(i, tick)| {
            [
                source(case, old, &signs[i * 2], revision, *tick),
                source(case, new, &signs[i * 2 + 1], revision, *tick),
            ]
        })
        .collect();
    let demand = CostDemand::admit(&case.expanded.gears[0], &[]).unwrap();
    let forecast = |ns| {
        FulfillmentCostForecast::sequential(
            CostForecastStages {
                setup: Some(
                    CostDurationRange::admit(duration(0), duration(0), &mut work()).unwrap(),
                ),
                queue: Some(
                    CostDurationRange::admit(duration(0), duration(0), &mut work()).unwrap(),
                ),
                service: Some(
                    CostDurationRange::admit(duration(ns), duration(ns), &mut work()).unwrap(),
                ),
                transfer: Some(
                    CostDurationRange::admit(duration(0), duration(0), &mut work()).unwrap(),
                ),
            },
            &mut work(),
        )
        .unwrap()
    };
    let models: Vec<_> = old_ns
        .iter()
        .flat_map(|n| [forecast(*n), forecast(10)])
        .collect();
    let forecasts: Vec<_> = sources
        .iter()
        .zip(&models)
        .map(|(s, m)| CostedForecast::bind(s, demand, m).unwrap())
        .collect();
    let observations: Vec<_> = forecasts
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| ReplanCostObservation {
            previous: &p[0],
            candidate: &p[1],
        })
        .collect();
    let calibrations = [revision];
    let now = instant(case, now);
    let context = CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    gate.evaluate(
        policy(),
        facts,
        &observations,
        &context,
        &mut ExactNumericWork::new(budget),
    )
}
pub(super) fn evaluate(
    gate: &mut ReluctantReplanGate,
    case: &Cases,
    facts: &ReplacementCostFacts<'_>,
    ticks: &[u64],
    old_ns: &[u64],
    now: u64,
) -> Result<ReluctantReplanDecision, ReluctantReplanRefusal> {
    evaluate_with_budget(gate, case, facts, ticks, old_ns, now, 1_000_000_000)
}
