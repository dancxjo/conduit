//! Deterministic catalogue/provider fixtures through the ordinary pool runtime.
//! This proves demand and projection custody, not live Ollama performance.
mod common;
#[allow(dead_code)] // The existing pool suite also uses the session-plan helpers.
#[path = "common/local_model_pool.rs"]
mod pool_fixture;
use conduit_core::*;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_planner::resource_cost::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn time(n: i128, d: i128) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::ratio(n, d, &mut work()).unwrap(),
        Unit::resolve("s").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap()
}
fn point(n: i128, d: i128) -> CostDurationRange {
    let duration = CostDuration::admit(time(n, d), &mut work()).unwrap();
    CostDurationRange::admit(duration, duration, &mut work()).unwrap()
}
fn window(host: &HostAdvertisement, observed: u64, expires: u64) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "pool/producer-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(observed, clock.clone()).unwrap(),
        MonotonicInstant::new(expires, clock).unwrap(),
    )
    .unwrap()
}
fn prompt_codec(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    validate_primitive_info(TEXT_INFO_ID, bytes).map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn input_bytes(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    u64::try_from(demand.input_bytes()).map_err(|_| CostEvidenceRefusal::WorkLimit)
}

fn bind_sources<'a>(
    hosts: &'a [HostAdvertisement],
    signs: &'a [SignId],
    revision: CostCalibrationRevision<'a>,
    observed: u64,
    expires: u64,
) -> Vec<CostEvidenceSource<'a>> {
    hosts
        .iter()
        .zip(signs)
        .map(|(host, sign)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                sign,
                revision,
                window(host, observed, expires),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn workload_forecast_and_independent_current_state_reach_atomic_pool_selection() {
    let providers = common::local_model::dual_local_model_providers();
    let expanded = pool_fixture::expanded();
    let hosts = vec![
        pool_fixture::consumer_host(&expanded.gears[0].checked_front()),
        providers[0].advertisement.clone(),
        providers[1].advertisement.clone(),
    ];
    let original = pool_fixture::build_plan(&hosts);
    assert!(original.activations.is_empty());
    let mut fragments = original.fragments.clone();
    fragments
        .iter_mut()
        .flat_map(|f| &mut f.shared_pools)
        .for_each(|p| {
            p.selection_policy = SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity
        });
    // The fixture selects the new policy before sealing its new Plan. Ordinary
    // configurable pool policy construction remains a separate product task.
    let plan = seal_plan_with_realization_backs_and_completion(
        PlotIdentity {
            source_document_id: original.source_document_id.clone(),
            checked_plot_id: original.checked_plot_id.clone(),
            expanded_plot_id: original.expanded_plot_id.clone(),
        },
        original.completion_policy,
        original.realization_backs.clone(),
        fragments,
    );
    assert!(verify_plan(&plan));
    let pool_id = plan.fragments[0].shared_pools[0].pool_id.clone();
    let members = &plan.fragments[0].shared_pools[0].realization_envelope;
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    let gear = conduit_plot::parse_with_startup(
        "plot answer {\n model: llm/generate(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 1024, maximum-work-units = 4096, maximum-history-items = 0)\n}\n",
        &startup, &profile).unwrap().gears.remove(0);
    let kind = KindId::from(conduit_ai::GENERATION_REQUEST_VALUE_KIND);
    let codec_revision = ArtifactId::from("fixture/pool-prompt-codec@1");
    let codecs = [CostInputCodec::admit(&kind, &codec_revision, 4096, prompt_codec).unwrap()];
    let value = ValuePayload {
        value_kind: kind.clone(),
        encoded: vec![b'p'],
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let metric_id = ArtifactId::from("fixture/pool-input-bytes@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 4096, input_bytes).unwrap();
    let request = CostDemand::admit_with_catalog(&gear, &inputs, &[], &codecs)
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    let calibration_id = ArtifactId::from("fixture/pool-calibration@1");
    let revision = CostCalibrationRevision::new(&calibration_id, 1).unwrap();
    let signs = [
        SignId::from("forecast/compact"),
        SignId::from("forecast/wide"),
    ];
    let historical_signs = [
        SignId::from("execution/historical-compact"),
        SignId::from("execution/historical-wide"),
    ];
    let current_signs = [
        SignId::from("execution/current-compact"),
        SignId::from("execution/current-wide"),
    ];
    let sources = bind_sources(&hosts[1..], &signs, revision, 10, 30);
    let historical_sources = bind_sources(&hosts[1..], &historical_signs, revision, 1, 2);
    let current_sources = bind_sources(&hosts[1..], &current_signs, revision, 12, 19);
    let condition = CostExecutionCondition::Warm {
        residency_revision: 7,
    };
    let historical = historical_sources
        .iter()
        .map(|s| CostExecutionObservation::admit(s, condition).unwrap())
        .collect::<Vec<_>>();
    let current = current_sources
        .iter()
        .map(|s| CostExecutionObservation::admit(s, condition).unwrap())
        .collect::<Vec<_>>();
    let models = [
        CostServiceModel::admit(
            &sources[0],
            request.class(),
            &metric,
            point(1, 3),
            CostNonServiceStages {
                setup: Some(point(0, 1)),
                queue: Some(point(0, 1)),
                transfer: Some(point(0, 1)),
            },
        )
        .unwrap(),
        CostServiceModel::admit(
            &sources[1],
            request.class(),
            &metric,
            point(1, 2),
            CostNonServiceStages {
                setup: Some(point(0, 1)),
                queue: Some(point(0, 1)),
                transfer: Some(point(0, 1)),
            },
        )
        .unwrap(),
    ];
    let predicted = models
        .iter()
        .map(|m| m.predict(request, &mut work()).unwrap())
        .collect::<Vec<_>>();
    let forecasts = predicted
        .iter()
        .zip(&historical)
        .map(|(f, h)| f.bind().unwrap().with_execution_assumption(h).unwrap())
        .collect::<Vec<_>>();
    let now = hosts[1..]
        .iter()
        .map(|h| MonotonicInstant::new(15, window(h, 10, 30).observed().clock().clone()).unwrap())
        .collect::<Vec<_>>();
    let operation = PoolOperationId::from("operation/exact-work");
    let prepare = |index: usize, request, budget: &mut ExactNumericWork| {
        prepare_pool_cost_forecast(
            PoolForecastPreparation {
                plan: &plan,
                fragment_index: 0,
                pool: &pool_id,
                operation: &operation,
                demand_identity: [9; 32],
                realization: members
                    .iter()
                    .position(|r| r.host_id == hosts[index + 1].host_id)
                    .unwrap() as u16,
                request,
                forecast: &forecasts[index],
                current_host: &hosts[index + 1],
                current_calibration: revision,
                now: &now[index],
                current_execution: Some(&current[index]),
                tolerance: time(1, 1_000_000_000),
            },
            budget,
        )
    };
    let prepared = (0..2)
        .map(|i| prepare(i, request, &mut work()).ok().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(prepared[0].cost.duration().upper_nanoseconds(), 333_333_334);
    assert_eq!(prepared[0].account.work.as_ref().unwrap().3, 1);
    assert_eq!(prepared[0].account.input_bytes, 1);
    assert_eq!(
        prepared[0]
            .account
            .stages
            .service
            .unwrap()
            .upper()
            .seconds(),
        ExactRational::ratio(1, 3, &mut work()).unwrap()
    );
    assert_eq!(prepared[0].cost.observed().ticks(), 10);
    assert_eq!(prepared[0].cost.expires().ticks(), 30);
    assert_eq!(
        prepared[0]
            .account
            .execution_assumption
            .as_ref()
            .unwrap()
            .0
            .window
            .expires()
            .ticks(),
        2
    );
    assert!(prepared[0]
        .cost
        .supporting_signs()
        .contains(&historical_signs[0]));
    let changed_value = ValuePayload {
        value_kind: kind.clone(),
        encoded: vec![b'p', b'p'],
    };
    let changed_inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &changed_value,
    }];
    let changed_request = CostDemand::admit_with_catalog(&gear, &changed_inputs, &[], &codecs)
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    let mut budget = work();
    assert!(matches!(
        prepare(0, changed_request, &mut budget),
        Err(PoolForecastPreparationRefusal::Forecast(
            CostEvidenceRefusal::WorkExtentChanged
        ))
    ));
    assert_eq!(budget.spent(), 0);
    assert!(matches!(
        prepare(0, request, &mut ExactNumericWork::new(0)),
        Err(PoolForecastPreparationRefusal::Preparation(
            PoolCostPreparationRefusal::Duration(_)
        ))
    ));
    let costs = prepared.into_iter().map(|p| p.cost).collect::<Vec<_>>();
    let calibration = [PoolCostCalibrationRevision::new(calibration_id.clone(), 1).unwrap()];
    let execution = current
        .iter()
        .map(|e| PoolCostExecutionEvaluation {
            host: e.source().host(),
            offer: e.source().offer(),
            resource: e.source().resource(),
            calibration: &calibration[0],
            sign: e.source().sign(),
            observed: e.source().window().observed(),
            expires: e.source().window().expires(),
            condition: e.condition(),
        })
        .collect::<Vec<_>>();
    let evaluations = hosts[1..]
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibration,
            executions: &execution,
        })
        .collect::<Vec<_>>();
    let observations = members
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let host = hosts.iter().find(|h| h.host_id == r.host_id).unwrap();
            PoolRealizationObservation {
                host_id: r.host_id.clone(),
                boot_id: r.boot_id.clone(),
                offer_generation: r.offer_generation,
                capability_id: r.capability_id.clone(),
                implementation_id: r.implementation_id.clone(),
                artifact_id: r.artifact_id.clone(),
                health: PoolRealizationHealth::Ready,
                sign_id: format!("current/member-{i}").into(),
                resources: common::resource_observations(std::slice::from_ref(host)),
            }
        })
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 256, 32>::new(&plan, 0, pool_id, NodeId(10), 1).unwrap();
    let selected = pool
        .admit_new_operation_with_costs(
            operation,
            MemberKey([1; 32]),
            &observations,
            "selection/exact-work".into(),
            [9; 32],
            &costs,
            &evaluations,
        )
        .unwrap();
    let selected_index = members
        .iter()
        .position(|r| r.host_id == hosts[1].host_id)
        .unwrap() as u16;
    assert!(
        matches!(selected.admission, SharedPoolAdmissionOutcome::Selected { member, evidence }
        if member.placement.realization == selected_index && evidence.observation_sign_ids.contains(&historical_signs[0]) && evidence.observation_sign_ids.contains(&current_signs[0]))
    );
    assert_eq!(pool.population(), 1);
}
