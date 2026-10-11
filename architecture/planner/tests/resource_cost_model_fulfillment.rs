//! Deterministic LocalModelOffer fixtures, not live Ollama timing evidence.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn prompt_codec(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    // The hosted Ollama generation adapter consumes bounded UTF-8 prompts.
    // Reuse the semantic Text validator for this reviewed fixture catalogue.
    conduit_core::validate_primitive_info(TEXT_INFO_ID, bytes)
        .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn input_bytes(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    if demand.class().gear().kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND {
        return Err(CostEvidenceRefusal::InvalidWorkInput);
    }
    u64::try_from(demand.input_bytes()).map_err(|_| CostEvidenceRefusal::WorkLimit)
}
fn point(n: i128, d: i128, work: &mut ExactNumericWork) -> CostDurationRange {
    let value = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(n, d, work).unwrap(),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        work,
    )
    .unwrap();
    CostDurationRange::admit(value, value, work).unwrap()
}
fn window(host: &HostAdvertisement) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "fixture/model-cost-clock".into(),
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
    bytes: usize,
    withdraw_compact: bool,
    inspect: impl FnOnce(CostRealizationSelection<'_>),
) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    let gear = conduit_plot::parse_with_startup(
        "plot answer {\n model: llm/generate(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 1024, maximum-work-units = 4096, maximum-history-items = 0)\n}\n",
        &startup, &profile,
    ).unwrap().gears.remove(0);
    let providers = common::local_model::dual_local_model_providers();
    let hosts = providers.map(|provider| provider.advertisement);
    assert_eq!(
        hosts[0].capabilities[0].inputs,
        hosts[1].capabilities[0].inputs
    );
    assert_eq!(
        hosts[0].capabilities[0].kind_contract_revision,
        hosts[1].capabilities[0].kind_contract_revision
    );
    let kind = KindId::from(conduit_ai::GENERATION_REQUEST_VALUE_KIND);
    let codec_revision = ArtifactId::from("fixture/ollama-utf8-prompt@1");
    let codecs = [CostInputCodec::admit(&kind, &codec_revision, 4096, prompt_codec).unwrap()];
    let value = ValuePayload {
        value_kind: kind.clone(),
        encoded: vec![b'p'; bytes],
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let metric_id = ArtifactId::from("fixture/model-input-byte@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 4096, input_bytes).unwrap();
    let demand = CostDemand::admit_with_catalog(&gear, &inputs, &[], &codecs)
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    assert_eq!(demand.work_estimate().unwrap().units(), bytes as u64);
    let profile = ArtifactId::from("fixture/model-byte-service-profile@1");
    let revisions = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let signs: [SignId; 2] = [
        "fixture/model/warm-compact",
        "fixture/model/cold-congested-wide",
    ]
    .map(Into::into);
    let sources = hosts
        .iter()
        .zip(&signs)
        .map(|(host, sign)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                sign,
                revisions[0],
                window(host),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut work = ExactNumericWork::new(100_000_000);
    let zero = point(0, 1, &mut work);
    // Fixture observations: compact is resident/lightly loaded; wide has a
    // setup and queue penalty but lower service Time per input byte. The fixed
    // output/work limits are part of the checked work class, not an inferred
    // token count. These are supplied estimates, not measured live rates.
    let compact = CostServiceModel::admit(
        &sources[0],
        demand.class(),
        &metric,
        point(1, 1, &mut work),
        CostNonServiceStages {
            setup: Some(point(1, 1, &mut work)),
            queue: Some(zero),
            transfer: Some(zero),
        },
    )
    .unwrap();
    let wide = CostServiceModel::admit(
        &sources[1],
        demand.class(),
        &metric,
        point(1, 10, &mut work),
        CostNonServiceStages {
            setup: Some(point(1000, 1, &mut work)),
            queue: Some(point(200, 1, &mut work)),
            transfer: Some(zero),
        },
    )
    .unwrap();
    let prepared = [
        compact.predict(demand, &mut work).unwrap(),
        wide.predict(demand, &mut work).unwrap(),
    ];
    let samples = prepared
        .iter()
        .map(|estimate| estimate.bind().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        samples[0]
            .forecast()
            .stages()
            .service
            .unwrap()
            .lower()
            .original()
            .coordinate(),
        ExactRational::from_i128(bytes as i128)
    );
    let mut observations = common::resource_observations(&hosts);
    if withdraw_compact {
        for observation in &mut observations {
            if observation.host_id == hosts[0].host_id
                && observation.class_id.as_str() == conduit_ai::LOCAL_MODEL_COMPUTE_RESOURCE
            {
                observation.unreserved_units = 0;
            }
        }
    }
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
                intent: if bytes < 100 {
                    PerformanceIntent::Interactive
                } else {
                    PerformanceIntent::ThroughputBatch
                },
                unavailable: UnavailableCostPolicy::RequireKnown,
            },
            &mut work,
        )
        .unwrap(),
    );
}
#[test]
fn short_warm_model_job_beats_wide_cold_host_but_larger_job_reverses() {
    exercise(64, false, |selection| {
        assert_eq!(
            selection.realization.choice.host_id.as_str(),
            "host/compact"
        );
        assert!(selection.used_cost_preference);
        assert_eq!(
            selection.costs[0].submitted_source.unwrap().sign().as_str(),
            "fixture/model/warm-compact"
        );
    });
    exercise(2048, false, |selection| {
        assert_eq!(selection.realization.choice.host_id.as_str(), "host/wide");
        assert!(selection.used_cost_preference);
        let CostAvailability::Known(sample) = selection.costs[1].availability else {
            panic!("known forecast")
        };
        assert_eq!(sample.demand().work_estimate().unwrap().units(), 2048);
        assert_eq!(
            sample.upper_seconds(),
            ExactRational::ratio(878, 625, &mut ExactNumericWork::new(100_000_000)).unwrap()
        );
    });
}
#[test]
fn favorable_model_estimate_cannot_override_withdrawn_compute_capacity() {
    exercise(64, true, |selection| {
        assert_eq!(selection.realization.choice.host_id.as_str(), "host/wide");
        assert!(selection
            .realization
            .signs
            .iter()
            .any(|record| record.host_id.as_str() == "host/compact"
                && record.disposition
                    == RealizationDecisionDisposition::Rejected(
                        RealizationRejection::CurrentResourceObservation
                    )));
    });
}
