use super::*;

#[test]
fn actual_native_completion_joins_atomic_dispatch_and_rejects_wrong_input() {
    for terminal in [
        LocalModelAdapterTerminal::Produced,
        LocalModelAdapterTerminal::Truncated,
        LocalModelAdapterTerminal::Refused,
        LocalModelAdapterTerminal::Failed,
        LocalModelAdapterTerminal::Cancelled,
        LocalModelAdapterTerminal::ProviderLost,
        LocalModelAdapterTerminal::InvalidStructuredResult,
    ] {
        inspect_terminal(terminal);
    }
}
fn inspect_terminal(terminal: LocalModelAdapterTerminal) {
    use conduit_kernel::{shared_pool::MemberKey, NodeId};
    use conduit_observatory::CostExplanationRefusal;
    use conduit_plan_lowering::shared_pool_runtime::*;
    let NativeFixture {
        mut host,
        gear,
        mut plan,
        consumer,
        ..
    } = native_fixture_for_terminal(terminal);
    for fragment in &mut plan.fragments {
        fragment.shared_pools[0].selection_policy =
            SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity;
    }
    plan = seal_plan(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.fragments,
    );
    let advertisement = host.advertisement().clone();
    let pool_id = plan.fragments[0].shared_pools[0].pool_id.clone();
    let envelope = &plan.fragments[0].shared_pools[0].realization_envelope[0];
    let offer = advertisement
        .capabilities
        .iter()
        .find(|offer| offer.capability_id == envelope.capability_id)
        .unwrap();
    let clock = MonotonicClockIdentity::new(
        advertisement.host_id.clone(),
        advertisement.boot_id.clone(),
        "fixture/forecast-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let now = MonotonicInstant::new(15, clock.clone()).unwrap();
    let calibration_id = ArtifactId::from("fixture/inspection-calibration@1");
    let revision = CostCalibrationRevision::new(&calibration_id, 1).unwrap();
    let sign = SignId::from("fixture/inspection-forecast");
    let source = CostEvidenceSource::bind(
        &advertisement,
        offer,
        None,
        &sign,
        revision,
        CostObservationWindow::admit(
            MonotonicInstant::new(10, clock.clone()).unwrap(),
            MonotonicInstant::new(30, clock).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let metric_id = ArtifactId::from("fixture/input-bytes@1");
    let metric =
        CostWorkMetric::admit(&metric_id, 1, 4096, |d| Ok(d.input_bytes() as u64)).unwrap();
    let numeric = || ExactNumericWork::new(100_000_000);
    let time = |n| {
        RationalQuantity::new(
            ExactRational::from_i128(n),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap()
    };
    let point = |n| {
        let duration = CostDuration::admit(time(n), &mut numeric()).unwrap();
        CostDurationRange::admit(duration, duration, &mut numeric()).unwrap()
    };
    with_demand(&gear, b"custody", |demand| {
        let demand = demand.with_work_metric(&metric).unwrap();
        let model = CostServiceModel::admit(
            &source,
            demand.class(),
            &metric,
            point(1),
            CostNonServiceStages {
                setup: Some(point(0)),
                queue: Some(point(0)),
                transfer: Some(point(0)),
            },
        )
        .unwrap();
        let prediction = model.predict(demand, &mut numeric()).unwrap();
        let forecast = prediction.bind().unwrap();
        let operation = PoolOperationId::from("fixture/inspection-request");
        let prepared = prepare_pool_cost_forecast(
            PoolForecastPreparation {
                plan: &plan,
                fragment_index: 0,
                pool: &pool_id,
                operation: &operation,
                demand_identity: [7; 32],
                realization: 0,
                request: demand,
                forecast: &forecast,
                current_host: &advertisement,
                current_calibration: revision,
                now: &now,
                current_execution: None,
                tolerance: RationalQuantity::new(
                    ExactRational::ratio(1, 1_000_000_000, &mut numeric()).unwrap(),
                    Unit::resolve("s").unwrap(),
                    QuantityRole::Linear,
                )
                .unwrap(),
            },
            &mut numeric(),
        )
        .unwrap();
        let observation = PoolRealizationObservation {
            host_id: envelope.host_id.clone(),
            boot_id: envelope.boot_id.clone(),
            offer_generation: envelope.offer_generation,
            capability_id: envelope.capability_id.clone(),
            implementation_id: envelope.implementation_id.clone(),
            artifact_id: envelope.artifact_id.clone(),
            health: PoolRealizationHealth::Ready,
            sign_id: "fixture/current".into(),
            resources: advertisement
                .resources
                .iter()
                .enumerate()
                .map(|(i, r)| ResourceObservation {
                    host_id: advertisement.host_id.clone(),
                    boot_id: advertisement.boot_id.clone(),
                    offer_generation: advertisement.offer_generation,
                    pool_id: r.pool_id.clone(),
                    class_id: r.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units: r.capacity_units,
                    utilized_units: 0,
                    sign_id: format!("fixture/resource-{i}").into(),
                })
                .collect(),
        };
        let revisions = [PoolCostCalibrationRevision::new(calibration_id.clone(), 1).unwrap()];
        let evaluations = [PoolCostEvaluation {
            host: &advertisement,
            now: &now,
            calibrations: &revisions,
            executions: &[],
        }];
        let mut pool =
            HostedSharedPool::<1, 256, 32>::new(&plan, 0, pool_id.clone(), NodeId(10), 1).unwrap();
        let dispatch = pool
            .admit_new_operation_with_costs(
                operation,
                MemberKey([7; 32]),
                &[observation],
                "fixture/inspection-selection".into(),
                [7; 32],
                &[prepared.cost],
                &evaluations,
            )
            .unwrap();
        let mut member = host
            .prepare_local_model_pool_member_with_demand(
                &plan,
                dispatch.account.evidence.clone(),
                &consumer,
                demand,
            )
            .unwrap();
        let mut output = Vec::with_capacity(4096);
        assert_eq!(
            host.execute_local_model_pool_member(&mut member, b"custody", &mut output),
            terminal
        );
        let completed = member.prepare_completed_work().unwrap();
        let mut accounts = [prepared.account];
        let report = completed
            .render_dispatch_inspection(&plan, &dispatch, &accounts, &mut numeric())
            .unwrap();
        assert!(report.contains(&format!("native_completion_outcome={terminal:?}")));
        assert!(report.contains("measured_scope=adapter-call"));
        assert!(report.contains("queue_latency=not-measured"));
        assert!(!report.contains("custody"));
        println!("\n{report}");
        let population = pool.population();
        for _ in 0..8 {
            assert_eq!(
                completed
                    .render_dispatch_inspection(&plan, &dispatch, &accounts, &mut numeric())
                    .unwrap(),
                report
            );
        }
        assert_eq!(pool.population(), population);
        let original_configuration = accounts[0].configuration_digest;
        let mut changed_configuration = completed.realization().configuration.clone();
        let bound = changed_configuration
            .iter_mut()
            .find(|entry| entry.key == "maximum-output-bytes")
            .unwrap();
        bound.value = ConfigurationValue::U64(512);
        accounts[0].configuration_digest =
            pool_operation_configuration_digest(&changed_configuration);
        assert_eq!(
            completed.render_dispatch_inspection(&plan, &dispatch, &accounts, &mut numeric()),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        accounts[0].configuration_digest = original_configuration;
        accounts[0].inputs[0].digest = pool_operation_input_digest(b"changed");
        assert_eq!(
            completed.render_dispatch_inspection(&plan, &dispatch, &accounts, &mut numeric()),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        host.release_local_model_pool_member(member).unwrap();
    });
}
