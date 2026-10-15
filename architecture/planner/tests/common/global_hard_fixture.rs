//! Current observed Host fixtures for ordinary global hard-admission tests.
use crate::common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use std::collections::BTreeMap;

fn window(host: &HostId, boot: &BootId) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.clone(),
        boot.clone(),
        "global-hard/clock".into(),
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
pub fn exercise(
    configure: impl FnOnce(&mut [HostAdvertisement], &mut HardRealizationRequirements),
    observe: impl FnOnce(&mut [ResourceObservation]),
    inspect: impl FnOnce(Result<GlobalPlacementResult, GlobalPlacementRefusal>, &[HostAdvertisement]),
) {
    let plot = conduit_plot::parse_with_startup(
        "plot hard {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let mut hosts = common::competing_hosts();
    let mut requirements = HardRealizationRequirements::default();
    configure(&mut hosts, &mut requirements);
    for host in &mut hosts {
        host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        for offer in &mut host.capabilities {
            offer
                .resource_requirements
                .sort_by(|a, b| a.class_id.cmp(&b.class_id));
        }
    }
    let windows: Vec<_> = hosts
        .iter()
        .map(|h| window(&h.host_id, &h.boot_id))
        .collect();
    let now: Vec<_> = windows
        .iter()
        .map(|w| MonotonicInstant::new(101, w.observed().clock().clone()).unwrap())
        .collect();
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            now,
            calibrations: &[],
            execution_observations: &[],
        })
        .collect();
    let policy = RealizationPolicy::default();
    let operations = [CostPlanningOperation {
        demand: CostDemand::admit(&plot.gears[0], &[]).unwrap(),
        forecasts: &[],
        requirements: &requirements,
        tie_policy: &policy,
        intent: PerformanceIntent::ThroughputBatch,
        unavailable: UnavailableCostPolicy::IgnoreCostsIfIncomplete,
    }];
    let mut observations = common::resource_observations(&hosts);
    observe(&mut observations);
    let observation_windows: Vec<_> = observations
        .iter()
        .map(|o| window(&o.host_id, &o.boot_id))
        .collect();
    let timed: Vec<_> = observations
        .iter()
        .zip(&observation_windows)
        .map(|(o, w)| TimedResourceObservation::admit(o, w.clone()).unwrap())
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let result = plan_global_realizations_with_costs(
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
        GlobalPlacementBudget { maximum_states: 16 },
        &mut ExactNumericWork::new(1_000_000),
    );
    if let Ok(result) = &result {
        assert!(verify_plan(&result.plan));
        assert!(result.states <= 16);
        assert_eq!(result.disposition, GlobalPlacementDisposition::Exhaustive);
        assert_eq!(result.score.unknown_operations, 1);
        for fragment in &result.plan.fragments {
            for placement in &fragment.placements {
                let host = hosts
                    .iter()
                    .find(|h| h.host_id == placement.host_id)
                    .unwrap();
                assert_eq!(placement.boot_id, host.boot_id);
                assert_eq!(placement.offer_generation, host.offer_generation);
                let offer = host
                    .capabilities
                    .iter()
                    .find(|c| c.capability_id == placement.capability_id)
                    .unwrap();
                assert_eq!(
                    placement.implementation_id,
                    offer.implementation.implementation_id
                );
                assert_eq!(placement.artifact_id, offer.implementation.artifact_id);
                assert_eq!(placement.kind_id, offer.kind_id);
                assert_eq!(
                    placement.kind_contract_revision,
                    offer.kind_contract_revision
                );
                assert_eq!(placement.inputs, offer.inputs);
                assert_eq!(placement.outputs, offer.outputs);
            }
        }
    }
    inspect(result, &hosts);
}
