//! Full common cost selection, occupied-capacity waiting and native delivery.
use super::*;
use crate::NativePoolPreparedCosts;

#[test]
fn cost_dispatch_retains_capacity_refusal_then_executes_each_exact_native_request() {
    use conduit_plan_lowering::shared_pool_runtime::{
        HostedSharedPool, PoolCostEvaluation, PoolWaitingLimits, PoolWaitingPolicy,
        SharedPoolAdmissionOutcome,
    };
    let mut fixture = waiting_fixture();
    let gear = fixture.gear.clone();
    with_demand(&gear, b"request", |first| {
        with_demand(&gear, b"later", |second| {
            let a = metadata(&fixture, first.input_bytes());
            let mut b = metadata(&fixture, second.input_bytes());
            b.operation = "fixture/second".into();
            b.key = conduit_kernel::shared_pool::MemberKey([9; 32]);
            b.demand = [10; 32];
            let now = MonotonicInstant::new(0, a.waiting_expires.clock().clone()).unwrap();
            let advertised = fixture.host.advertisement().clone();
            let native_clock = MonotonicClockIdentity::new(
                advertised.host_id.clone(),
                advertised.boot_id.clone(),
                "fixture/native-cost-clock".into(),
                TemporalScale::Microseconds,
                1,
                0,
            )
            .unwrap();
            let native_now = MonotonicInstant::new(0, native_clock.clone()).unwrap();
            let native_expiry = MonotonicInstant::new(1000, native_clock).unwrap();
            let offer = advertised
                .capabilities
                .iter()
                .find(|offer| offer.kind_id == gear.kind_id)
                .unwrap();
            let calibration = ArtifactId::from("fixture/custody-calibration@1");
            let revision = CostCalibrationRevision::new(&calibration, 1).unwrap();
            let sign = SignId::from("fixture/custody-forecast");
            let source = CostEvidenceSource::bind(
                &advertised,
                offer,
                None,
                &sign,
                revision,
                CostObservationWindow::admit(native_now.clone(), native_expiry).unwrap(),
            )
            .unwrap();
            let mut work = ExactNumericWork::new(100_000_000);
            let point = |us, work: &mut ExactNumericWork| {
                let time = CostDuration::admit(
                    RationalQuantity::new(
                        ExactRational::from_i128(us),
                        Unit::resolve("us").unwrap(),
                        QuantityRole::Linear,
                    )
                    .unwrap(),
                    work,
                )
                .unwrap();
                CostDurationRange::admit(time, time, work).unwrap()
            };
            let stages = FulfillmentCostForecast::sequential(
                CostForecastStages {
                    setup: Some(point(0, &mut work)),
                    queue: Some(point(0, &mut work)),
                    service: Some(point(1, &mut work)),
                    transfer: Some(point(0, &mut work)),
                },
                &mut work,
            )
            .unwrap();
            let first_forecast = CostedForecast::bind(&source, first, &stages).unwrap();
            let second_forecast = CostedForecast::bind(&source, second, &stages).unwrap();
            let tolerance = RationalQuantity::new(
                ExactRational::ZERO,
                Unit::resolve("s").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap();
            let revisions = [PoolCostCalibrationRevision::new(calibration.clone(), 1).unwrap()];
            let evaluations = [PoolCostEvaluation {
                host: &advertised,
                now: &native_now,
                calibrations: &revisions,
                executions: &[],
            }];
            let realization = &fixture.plan.fragments[0].shared_pools[0].realization_envelope[0];
            let resource_signs: Vec<SignId> = realization
                .resources
                .iter()
                .enumerate()
                .map(|(index, _)| format!("fixture/resource/{index}").into())
                .collect();
            let current = [fixture
                .host
                .observe_local_model_pool_realization(
                    realization,
                    "fixture/provider".into(),
                    &resource_signs,
                )
                .unwrap()];
            let pool = HostedSharedPool::<2, 64, 8>::new(
                &fixture.plan,
                0,
                fixture.selection.pool_id.clone(),
                conduit_kernel::NodeId(10),
                1,
            )
            .unwrap();
            let mut queue = NativePoolRequestQueue::prepare(
                pool,
                PoolWaitingLimits {
                    operations: 2,
                    per_requester: 2,
                    admitted_bytes: 12,
                },
                PoolWaitingPolicy::Fifo,
                now.clone(),
            )
            .unwrap();
            queue
                .enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, a.clone(), first).unwrap(),
                    &now,
                )
                .unwrap();
            queue
                .enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, b.clone(), second).unwrap(),
                    &now,
                )
                .unwrap();
            let costs = NativePoolPreparedCosts::prepare(
                queue.next_request(&now).unwrap().unwrap(),
                vec![PoolForecastPreparation {
                    plan: &fixture.plan,
                    fragment_index: 0,
                    pool: &a.pool,
                    operation: &a.operation,
                    demand_identity: a.demand,
                    realization: 0,
                    request: first,
                    forecast: &first_forecast,
                    current_host: &advertised,
                    current_calibration: revision,
                    now: &native_now,
                    current_execution: None,
                    tolerance,
                }],
                &mut work,
            )
            .unwrap();
            let dispatched = queue
                .dispatch(
                    &now,
                    &current,
                    "fixture/select-first".into(),
                    &costs,
                    &evaluations,
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                dispatched.admission.outcome.account.choice,
                Some(PoolCostChoiceBasis::MinimumPredictedCompletion)
            );
            let SharedPoolAdmissionOutcome::Selected { member, evidence } =
                dispatched.admission.outcome.admission
            else {
                panic!("known eligible native request was not selected");
            };
            let request = dispatched.request.unwrap();
            assert_eq!(request.payload().encoded, b"request");
            let mut native = fixture
                .host
                .prepare_local_model_pool_member_with_demand(
                    &fixture.plan,
                    evidence,
                    &fixture.consumer,
                    request.demand(),
                )
                .unwrap();
            queue.trigger(member).unwrap();
            let mut output = Vec::with_capacity(4096);
            assert_eq!(
                fixture.host.execute_local_model_pool_member(
                    &mut native,
                    &request.payload().encoded,
                    &mut output
                ),
                LocalModelAdapterTerminal::Produced
            );
            assert_eq!(fixture.state.lock().unwrap().0, 1);
            assert!(matches!(
                queue.dispatch(
                    &now,
                    &current,
                    "fixture/wrong-request-cost".into(),
                    &costs,
                    &evaluations
                ),
                Err(NativePoolQueueError::Correlation)
            ));
            let second_costs = NativePoolPreparedCosts::prepare(
                queue.next_request(&now).unwrap().unwrap(),
                vec![PoolForecastPreparation {
                    plan: &fixture.plan,
                    fragment_index: 0,
                    pool: &b.pool,
                    operation: &b.operation,
                    demand_identity: b.demand,
                    realization: 0,
                    request: second,
                    forecast: &second_forecast,
                    current_host: &advertised,
                    current_calibration: revision,
                    now: &native_now,
                    current_execution: None,
                    tolerance,
                }],
                &mut work,
            )
            .unwrap();
            let refused = queue
                .dispatch(
                    &now,
                    &current,
                    "fixture/capacity-refusal".into(),
                    &second_costs,
                    &evaluations,
                )
                .unwrap()
                .unwrap();
            assert!(
                matches!(refused.admission.outcome.admission, SharedPoolAdmissionOutcome::Refused(ref evidence)
                if evidence.disposition == PoolSelectionDisposition::CapacityRefused)
            );
            assert!(refused.request.is_none());
            assert_eq!(queue.waiting_population(), 1);
            assert_eq!(
                queue.next_request(&now).unwrap().unwrap().payload().encoded,
                b"later"
            );
            fixture
                .host
                .release_local_model_pool_member(native)
                .unwrap();
            queue.release(member).unwrap();
            let dispatched = queue
                .dispatch(
                    &now,
                    &current,
                    "fixture/select-second".into(),
                    &second_costs,
                    &evaluations,
                )
                .unwrap()
                .unwrap();
            let SharedPoolAdmissionOutcome::Selected { member, evidence } =
                dispatched.admission.outcome.admission
            else {
                panic!("released capacity did not admit the retained request");
            };
            let request = dispatched.request.unwrap();
            let mut native = fixture
                .host
                .prepare_local_model_pool_member_with_demand(
                    &fixture.plan,
                    evidence,
                    &fixture.consumer,
                    request.demand(),
                )
                .unwrap();
            queue.trigger(member).unwrap();
            assert_eq!(
                fixture.host.execute_local_model_pool_member(
                    &mut native,
                    &request.payload().encoded,
                    &mut output
                ),
                LocalModelAdapterTerminal::Produced
            );
            assert_eq!(fixture.state.lock().unwrap().0, 2);
            assert_eq!(queue.waiting_population(), 0);
            fixture
                .host
                .release_local_model_pool_member(native)
                .unwrap();
            queue.release(member).unwrap();
        });
    });
}
