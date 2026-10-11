//! The actual model → selection → Observatory path retains work accounting.
use crate::render_cost_selection_explanation;
use alloc::{vec, vec::Vec};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn events(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    if demand.class().gear().kind_id.as_str() != conduit_signal::PULSE_KIND {
        return Err(CostEvidenceRefusal::InvalidWorkInput);
    }
    demand
        .class()
        .gear()
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("count", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or(CostEvidenceRefusal::InvalidWorkInput)
}
#[test]
fn modeled_selection_explains_exact_work_measure_per_unit_time_and_total() {
    let gear = conduit_plot::parse_with_startup(
        "plot cost {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap()
    .gears
    .remove(0);
    let mut host = conduit_signal_conformance::pico_local_advertisement();
    host.capabilities = vec![host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id == gear.kind_id)
        .unwrap()
        .clone()];
    let hosts = [host];
    let host = &hosts[0];
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "fixture/work-report-clock".into(),
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
    let now = MonotonicInstant::new(150, clock).unwrap();
    let profile = ArtifactId::from("fixture/work-report-profile@1");
    let revisions = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let sign = SignId::from("fixture/work-report-source");
    let source = CostEvidenceSource::bind(
        host,
        &host.capabilities[0],
        None,
        &sign,
        revisions[0],
        window.clone(),
    )
    .unwrap();
    let metric_id = ArtifactId::from("fixture/pulse-event@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 100, events).unwrap();
    let demand = CostDemand::admit(&gear, &[])
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    let mut work = ExactNumericWork::new(100_000_000);
    let point = |n, d, work: &mut ExactNumericWork| {
        let duration = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::ratio(n, d, work).unwrap(),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            work,
        )
        .unwrap();
        CostDurationRange::admit(duration, duration, work).unwrap()
    };
    let zero = point(0, 1, &mut work);
    let model = CostServiceModel::admit(
        &source,
        demand.class(),
        &metric,
        point(1, 3, &mut work),
        CostNonServiceStages {
            setup: Some(point(1, 1, &mut work)),
            queue: Some(zero),
            transfer: Some(zero),
        },
    )
    .unwrap();
    let prepared = model.predict(demand, &mut work).unwrap();
    let samples = [prepared.bind().unwrap()];
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: alloc::format!("fixture/hard-{index}").into(),
        })
        .collect::<Vec<_>>();
    let timed = observations
        .iter()
        .map(|observation| TimedResourceObservation::admit(observation, window.clone()).unwrap())
        .collect::<Vec<_>>();
    let contexts = [CostEvaluationContext {
        host,
        calibrations: &revisions,
        now: &now,
    }];
    let requirements = HardRealizationRequirements::default();
    let policy = RealizationPolicy::default();
    let selection = select_realization_with_costs(
        demand,
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &timed,
            tie_policy: &policy,
            forecasts: &samples,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::RequireKnown,
        },
        &mut work,
    )
    .unwrap();
    let output = render_cost_selection_explanation(&selection).unwrap();
    for required in [
        "work metric=fixture/pulse-event@1 revision=1 units=2 admitted_maximum_units=100",
        "stage service_per_work_unit=1/3 ms",
        "stage service=2/3 ms",
        "effective_fulfillment_seconds=[1/600,1/600]",
    ] {
        assert!(output.contains(required), "missing {required}: {output}");
    }
}
