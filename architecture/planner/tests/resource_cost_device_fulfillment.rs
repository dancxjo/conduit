//! Compute/printer-shaped catalogue fixtures exercise generic fulfillment laws.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

fn gear(kind: &str) -> conduit_plot::CheckedGear {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind.into(),
            kind_contract_revision: format!("{kind}@1").into(),
            inputs: vec![PortDescriptor {
                port_id: "work".into(),
                value_kind: COUNT_INFO_ID.into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    conduit_plot::parse_with_startup(
        &format!("plot device {{\n job: {kind}\n}}\n"),
        &startup,
        &profile,
    )
    .unwrap()
    .gears
    .remove(0)
}
fn count(demand: CostDemand<'_>, expected: &str) -> Result<u64, CostEvidenceRefusal> {
    if demand.class().gear().kind_id.as_str() != expected || demand.inputs().len() != 1 {
        return Err(CostEvidenceRefusal::InvalidWorkInput);
    }
    let input = demand.inputs()[0].value;
    if input.value_kind.as_str() != COUNT_INFO_ID {
        return Err(CostEvidenceRefusal::InvalidWorkInput);
    }
    Ok(u64::from_le_bytes(
        input
            .encoded
            .as_slice()
            .try_into()
            .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)?,
    ))
}
fn elements(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    count(demand, "fixture/compute-elements")
}
fn pages(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    count(demand, "fixture/print-pages")
}
fn point(n: i128, d: i128, work: &mut ExactNumericWork) -> CostDurationRange {
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
}
fn window(host: &HostAdvertisement) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "fixture/device-cost-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(200, clock).unwrap(),
    )
    .unwrap()
}
fn exercise(
    kind: &str,
    metric_name: &str,
    units: u64,
    models: [[i128; 4]; 2],
    inspect: impl FnOnce(CostRealizationSelection<'_>),
) {
    let gear = gear(kind);
    let metric_identity = ArtifactId::from(metric_name);
    let metric = CostWorkMetric::admit(
        &metric_identity,
        1,
        10_000,
        if kind == "fixture/print-pages" {
            pages
        } else {
            elements
        },
    )
    .unwrap();
    let value = ValuePayload {
        value_kind: COUNT_INFO_ID.into(),
        encoded: units.to_le_bytes().to_vec(),
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let raw = CostDemand::admit(&gear, &inputs).unwrap();
    let demand = raw.with_work_metric(&metric).unwrap();
    let wrong_identity = ArtifactId::from("fixture/other-domain-metric@1");
    let wrong = CostWorkMetric::admit(
        &wrong_identity,
        1,
        10_000,
        if kind == "fixture/print-pages" {
            elements
        } else {
            pages
        },
    )
    .unwrap();
    assert!(matches!(
        raw.with_work_metric(&wrong),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    let mut hosts = common::competing_hosts();
    for (index, host) in hosts.iter_mut().enumerate() {
        host.profile = "fixture/device-cost-host@1".into();
        host.resources = vec![resource_offer("fixture/service-slot", "fixture/service", 1)];
        let offer = &mut host.capabilities[0];
        offer.kind_id = gear.kind_id.clone();
        offer.kind_contract_revision = gear.kind_contract_revision.clone();
        offer.startup_parameters = gear.startup_parameters.clone();
        offer.inputs = gear.inputs.clone();
        offer.outputs = gear.outputs.clone();
        offer.semantic_contract = gear.semantic_contract.clone();
        offer.implementation.implementation_id = format!("fixture/device-{index}@1").into();
        offer.implementation.artifact_id = format!("fixture/device-artifact-{index}@1").into();
        offer.host_calls.clear();
        offer.resource_requirements = vec![resource_requirement("fixture/service", 1)];
        offer.authority_requirements.clear();
        offer.limits.max_queue_bytes = 8;
    }
    let profile = ArtifactId::from("fixture/device-service-profile@1");
    let revisions = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let signs: [SignId; 2] = ["fixture/cost/device-a", "fixture/cost/device-b"].map(Into::into);
    let sources = hosts
        .iter()
        .zip(&signs)
        .map(|(host, sign)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                Some(&host.resources[0]),
                sign,
                revisions[0],
                window(host),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut work = ExactNumericWork::new(100_000_000);
    let zero = point(0, 1, &mut work);
    let forecasts = sources
        .iter()
        .zip(models)
        .map(|(source, [setup, transfer, numerator, denominator])| {
            CostServiceModel::admit(
                source,
                demand.class(),
                &metric,
                point(numerator, denominator, &mut work),
                CostNonServiceStages {
                    setup: Some(point(setup, 1, &mut work)),
                    queue: Some(zero),
                    transfer: Some(point(transfer, 1, &mut work)),
                },
            )
            .unwrap()
            .predict(demand, &mut work)
            .unwrap()
        })
        .collect::<Vec<_>>();
    let samples = forecasts
        .iter()
        .map(|forecast| forecast.bind().unwrap())
        .collect::<Vec<_>>();
    let observations = common::resource_observations(&hosts);
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host)).unwrap()
        })
        .collect::<Vec<_>>();
    let times = hosts
        .iter()
        .map(|host| MonotonicInstant::new(150, window(host).observed().clock().clone()).unwrap())
        .collect::<Vec<_>>();
    let contexts = hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &revisions,
            now,
        })
        .collect::<Vec<_>>();
    let requirements = HardRealizationRequirements::default();
    let tie_policy = RealizationPolicy::default();
    inspect(
        select_realization_with_costs(
            demand,
            CostSelectionInputs {
                hosts: &hosts,
                contexts: &contexts,
                advertisements: &[],
                requirements: &requirements,
                observations: &timed,
                tie_policy: &tie_policy,
                forecasts: &samples,
                intent: PerformanceIntent::Interactive,
                unavailable: UnavailableCostPolicy::RequireKnown,
            },
            &mut work,
        )
        .unwrap(),
    );
}
#[test]
fn cpu_gpu_shaped_setup_and_transfer_costs_reverse_on_larger_supported_work() {
    let models = [[0, 0, 1, 1], [10, 10, 1, 10]];
    exercise(
        "fixture/compute-elements",
        "fixture/compute-element@1",
        1,
        models,
        |selection| {
            assert_eq!(
                selection.realization.choice.host_id.as_str(),
                "host-a-efficient"
            );
        },
    );
    exercise(
        "fixture/compute-elements",
        "fixture/compute-element@1",
        100,
        models,
        |selection| {
            assert_eq!(
                selection.realization.choice.host_id.as_str(),
                "host-b-capable"
            );
            let CostAvailability::Known(sample) = selection.costs[1].availability else {
                panic!("known")
            };
            assert_eq!(sample.demand().work_estimate().unwrap().units(), 100);
            assert_eq!(
                sample.upper_seconds(),
                ExactRational::ratio(3, 100, &mut ExactNumericWork::new(100_000_000)).unwrap()
            );
        },
    );
}
#[test]
fn printer_pages_are_declared_work_and_rates_become_fulfillment_time() {
    // 80 vs 120 pages/minute become 60000/80 vs 60000/120 ms/page.
    // The faster printer has 2 s setup; policy sees only the actual job's Time.
    let models = [[100, 0, 60_000, 80], [2000, 0, 60_000, 120]];
    exercise(
        "fixture/print-pages",
        "fixture/printed-page@1",
        2,
        models,
        |selection| {
            assert_eq!(
                selection.realization.choice.host_id.as_str(),
                "host-a-efficient"
            );
            let CostAvailability::Known(sample) = selection.costs[0].availability else {
                panic!("known")
            };
            assert_eq!(
                sample.upper_seconds(),
                ExactRational::ratio(8, 5, &mut ExactNumericWork::new(100_000_000)).unwrap()
            );
        },
    );
    exercise(
        "fixture/print-pages",
        "fixture/printed-page@1",
        20,
        models,
        |selection| {
            assert_eq!(
                selection.realization.choice.host_id.as_str(),
                "host-b-capable"
            );
            let CostAvailability::Known(sample) = selection.costs[1].availability else {
                panic!("known")
            };
            assert_eq!(
                sample
                    .demand()
                    .work_estimate()
                    .unwrap()
                    .metric_identity()
                    .as_str(),
                "fixture/printed-page@1"
            );
            assert_eq!(sample.upper_seconds(), ExactRational::from_i128(12));
        },
    );
}
