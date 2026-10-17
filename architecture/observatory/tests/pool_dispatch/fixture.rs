//! Supplied local-model facts through ordinary preparation and atomic admission.
use conduit_core::*;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_planner::resource_cost::*;
#[allow(dead_code)]
#[path = "../../../planner/tests/common/local_model_pool.rs"]
mod pool_fixture;
#[path = "../../../planner/tests/common/local_model.rs"]
mod providers;

pub type Pool = HostedSharedPool<2, 256, 32>;
#[derive(Copy, Clone)]
pub enum Case {
    Healthy,
    Stale,
    Missing,
    HardRefusal,
    Full,
    Queued,
}
pub fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
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
    let t = CostDuration::admit(time(n, d), &mut work()).unwrap();
    CostDurationRange::admit(t, t, &mut work()).unwrap()
}
fn codec(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    validate_primitive_info(TEXT_INFO_ID, bytes).map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn bytes(d: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    Ok(d.input_bytes() as u64)
}

pub fn scenario(
    case: Case,
    size: usize,
    inspect: impl FnOnce(
        &Plan,
        &mut Pool,
        &mut CostAwareSharedPoolAdmissionOutcome,
        &mut [PoolForecastPreparationAccount],
    ),
) {
    let providers = providers::dual_local_model_providers();
    let expanded = pool_fixture::expanded();
    let hosts = vec![
        pool_fixture::consumer_host(&expanded.gears[0].checked_front()),
        providers[0].advertisement.clone(),
        providers[1].advertisement.clone(),
    ];
    let plan = pool_fixture::build_plan_with_policy(
        &hosts,
        SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
    );
    let pool_id = plan.fragments[0].shared_pools[0].pool_id.clone();
    let members = &plan.fragments[0].shared_pools[0].realization_envelope;
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
    let gear=conduit_plot::parse_with_startup("plot answer {\n model: llm/generate(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 1024, maximum-work-units = 4096, maximum-history-items = 0)\n}\n",&startup,&profiles).unwrap().gears.remove(0);
    let kind = KindId::from(conduit_ai::GENERATION_REQUEST_VALUE_KIND);
    let revision_id = ArtifactId::from("inspection/codec@1");
    let codecs = [CostInputCodec::admit(&kind, &revision_id, 4096, codec).unwrap()];
    let payload = ValuePayload {
        value_kind: kind.clone(),
        encoded: vec![b'p'; size],
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &payload,
    }];
    let metric_id = ArtifactId::from("inspection/input-bytes@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 4096, bytes).unwrap();
    let demand = CostDemand::admit_with_catalog(&gear, &inputs, &[], &codecs)
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    let calibration_id = ArtifactId::from("inspection/calibration@1");
    let revision = CostCalibrationRevision::new(&calibration_id, 1).unwrap();
    let signs = [
        SignId::from("inspection/forecast-compact"),
        SignId::from("inspection/forecast-wide"),
    ];
    let clocks = hosts[1..]
        .iter()
        .map(|h| {
            MonotonicClockIdentity::new(
                h.host_id.clone(),
                h.boot_id.clone(),
                "inspection/clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let sources = hosts[1..]
        .iter()
        .enumerate()
        .map(|(i, h)| {
            CostEvidenceSource::bind(
                h,
                &h.capabilities[0],
                None,
                &signs[i],
                revision,
                CostObservationWindow::admit(
                    MonotonicInstant::new(10, clocks[i].clone()).unwrap(),
                    MonotonicInstant::new(30, clocks[i].clone()).unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let models = [
        CostServiceModel::admit(
            &sources[0],
            demand.class(),
            &metric,
            point(1, 3),
            CostNonServiceStages {
                setup: Some(point(0, 1)),
                queue: Some(point(1, 4)),
                transfer: Some(point(0, 1)),
            },
        )
        .unwrap(),
        CostServiceModel::admit(
            &sources[1],
            demand.class(),
            &metric,
            point(1, 10),
            CostNonServiceStages {
                setup: Some(point(1, 1)),
                queue: Some(point(0, 1)),
                transfer: Some(point(0, 1)),
            },
        )
        .unwrap(),
    ];
    let predicted = models
        .iter()
        .map(|m| m.predict(demand, &mut work()).unwrap())
        .collect::<Vec<_>>();
    let forecasts = predicted
        .iter()
        .map(|p| p.bind().unwrap())
        .collect::<Vec<_>>();
    let preparation_now = clocks
        .iter()
        .map(|c| MonotonicInstant::new(15, c.clone()).unwrap())
        .collect::<Vec<_>>();
    let prepare = |operation: &PoolOperationId| {
        (0..2)
            .map(|i| {
                prepare_pool_cost_forecast(
                    PoolForecastPreparation {
                        plan: &plan,
                        fragment_index: 0,
                        pool: &pool_id,
                        operation,
                        demand_identity: [9; 32],
                        realization: members
                            .iter()
                            .position(|m| m.host_id == hosts[i + 1].host_id)
                            .unwrap() as u16,
                        request: demand,
                        forecast: &forecasts[i],
                        current_host: &hosts[i + 1],
                        current_calibration: revision,
                        now: &preparation_now[i],
                        current_execution: None,
                        tolerance: time(1, 1_000_000_000),
                    },
                    &mut work(),
                )
                .unwrap()
            })
            .map(|p| (p.cost, p.account))
            .unzip::<_, _, Vec<_>, Vec<_>>()
    };
    let now = clocks
        .iter()
        .map(|c| {
            MonotonicInstant::new(if matches!(case, Case::Stale) { 31 } else { 15 }, c.clone())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let calibrations = [PoolCostCalibrationRevision::new(calibration_id.clone(), 1).unwrap()];
    let evaluations = hosts[1..]
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let mut observations = members
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let host = hosts.iter().find(|h| h.host_id == m.host_id).unwrap();
            PoolRealizationObservation {
                host_id: m.host_id.clone(),
                boot_id: m.boot_id.clone(),
                offer_generation: m.offer_generation,
                capability_id: m.capability_id.clone(),
                implementation_id: m.implementation_id.clone(),
                artifact_id: m.artifact_id.clone(),
                health: PoolRealizationHealth::Ready,
                sign_id: format!("inspection/current-{i}").into(),
                resources: host
                    .resources
                    .iter()
                    .enumerate()
                    .map(|(j, r)| ResourceObservation {
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        offer_generation: host.offer_generation,
                        pool_id: r.pool_id.clone(),
                        class_id: r.class_id.clone(),
                        health: ResourceHealth::Ready,
                        unreserved_units: r.capacity_units,
                        utilized_units: 0,
                        sign_id: format!("inspection/resource-{i}-{j}").into(),
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    if matches!(case, Case::HardRefusal) {
        observations[0].resources[0].unreserved_units = 0;
    }
    let mut pool = Pool::new(&plan, 0, pool_id.clone(), NodeId(10), 1).unwrap();
    if matches!(case, Case::Full) {
        for i in 1..=2 {
            let operation = PoolOperationId::from(format!("inspection/fill-{i}").as_str());
            let (costs, _) = prepare(&operation);
            let result = pool
                .admit_new_operation_with_costs(
                    operation,
                    MemberKey([i; 32]),
                    &observations,
                    format!("inspection/fill-sign-{i}").into(),
                    [9; 32],
                    &costs,
                    &evaluations,
                )
                .unwrap();
            assert!(matches!(
                result.admission,
                SharedPoolAdmissionOutcome::Selected { .. }
            ));
        }
    }
    let operation = PoolOperationId::from("inspection/request");
    let (mut costs, mut accounts) = prepare(&operation);
    if matches!(case, Case::Missing) {
        costs.clear();
        accounts.clear();
    }
    let mut result = if matches!(case, Case::Queued) {
        pool.enable_waiting(
            PoolWaitingLimits {
                operations: 2,
                per_requester: 2,
                admitted_bytes: 4096,
            },
            now[0].clone(),
        )
        .unwrap();
        pool.queue_operation(
            QueuedPoolOperation {
                plan: plan.plan_id.clone(),
                pool: pool_id,
                operation,
                requester: "inspection/requester".into(),
                class: PoolWaitingClass::Interactive,
                key: MemberKey([7; 32]),
                demand: [9; 32],
                admitted_bytes: size as u32,
                waiting_expires: MonotonicInstant::new(25, clocks[0].clone()).unwrap(),
            },
            &now[0],
        )
        .unwrap();
        pool.admit_next_waiting_with_costs(
            &now[0],
            &observations,
            "inspection/selection".into(),
            &costs,
            &evaluations,
        )
        .unwrap()
        .unwrap()
        .outcome
    } else {
        pool.admit_new_operation_with_costs(
            operation,
            MemberKey([7; 32]),
            &observations,
            "inspection/selection".into(),
            [9; 32],
            &costs,
            &evaluations,
        )
        .unwrap()
    };
    inspect(&plan, &mut pool, &mut result, &mut accounts);
}
