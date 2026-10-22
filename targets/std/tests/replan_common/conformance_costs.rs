//! Explicit fixed-clock conformance data, excluded from measurement workers.
use super::fixture::Case;
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_std_host::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn duration(ns: u64) -> CostDuration {
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
fn forecast(ns: u64) -> FulfillmentCostForecast {
    let range = |n| Some(CostDurationRange::admit(duration(n), duration(n), &mut work()).unwrap());
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: range(0),
            queue: range(0),
            service: range(ns),
            transfer: range(0),
        },
        &mut work(),
    )
    .unwrap()
}
pub fn with_request<R>(
    case: &mut Case,
    report: &StdRunReport,
    supplied: &[ExternalForeInput],
    old_ns: [u64; 3],
    now_tick: u64,
    run: impl FnOnce(&mut StdHost, StdReluctantReplanRequest<'_>) -> R,
) -> R {
    let ad = case.host.advertisement().clone();
    let clock = MonotonicClockIdentity::new(
        ad.host_id.clone(),
        ad.boot_id.clone(),
        "fixture/replan-clock".into(),
        TemporalScale::Nanoseconds,
        1,
        0,
    )
    .unwrap();
    let instant = |tick| MonotonicInstant::new(tick, clock.clone()).unwrap();
    let identity: ArtifactId = "fixture/replan-cost-profile".into();
    let revision = CostCalibrationRevision::new(&identity, 1).unwrap();
    let old = &case.previous.fragments[0].placements[0];
    let new = &case.candidate.fragments[0].placements[0];
    let source = |gear: &PlannedGear, sign, tick| {
        CostEvidenceSource::bind(
            &ad,
            ad.capabilities
                .iter()
                .find(|c| c.capability_id == gear.capability_id)
                .unwrap(),
            None,
            sign,
            revision,
            CostObservationWindow::admit(instant(tick), instant(10_000)).unwrap(),
        )
        .unwrap()
    };
    let stage_signs: Vec<SignId> = (0..5)
        .map(|i| format!("fixture/stage/{i}").into())
        .collect();
    let measurements: Vec<_> = (0..5)
        .map(|i| {
            CostMeasuredDuration::measure(
                &instant(100 + i * 10),
                &instant(101 + i * 10),
                &mut work(),
            )
            .unwrap()
        })
        .collect();
    let sources: Vec<_> = (0..5)
        .map(|i| {
            source(
                if i == 4 { old } else { new },
                &stage_signs[i],
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
                None,
            )
            .unwrap()
        })
        .collect();
    let calibrations = [revision];
    let now = instant(now_tick);
    let context = CostEvaluationContext {
        host: &ad,
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
        &[],
        std::slice::from_ref(&context),
        &mut work(),
    )
    .unwrap();
    let value = ValuePayload {
        value_kind: case.authored.expanded.gears[0].inputs[0].value_kind.clone(),
        encoded: 7u64.to_le_bytes().to_vec(),
    };
    let input = [CostInput {
        port: &case.authored.expanded.gears[0].inputs[0].port_id,
        value: &value,
    }];
    let demand = CostDemand::admit(&case.authored.expanded.gears[0], &input).unwrap();
    let signs: Vec<SignId> = (0..6)
        .map(|i| format!("fixture/forecast/{i}").into())
        .collect();
    let sampled: Vec<_> = (0..6)
        .map(|i| {
            source(
                if i % 2 == 0 { old } else { new },
                &signs[i],
                1000 + (i / 2) as u64 * 100,
            )
        })
        .collect();
    let models: Vec<_> = old_ns
        .into_iter()
        .flat_map(|n| [forecast(n), forecast(10)])
        .collect();
    let forecasts: Vec<_> = sampled
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
    let policy = ReluctantReplanPolicy::admit(
        3,
        duration(100),
        duration(5000),
        duration(250),
        duration(1000),
        duration(10),
        1,
        &mut work(),
    )
    .unwrap();
    run(
        &mut case.host,
        StdReluctantReplanRequest {
            previous_report: report,
            policy,
            costs: &facts,
            observations: &observations,
            context: &context,
            inputs: supplied,
        },
    )
}
