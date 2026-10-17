//! Inventory admission remains bounded and refuses nonadjacent duplicate IDs.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn select(count: usize, duplicate: bool, reverse: bool) -> Result<(), CostSelectionError> {
    let mut host = common::competing_hosts().into_iter().next().unwrap();
    let template = host.capabilities[0].clone();
    let unrelated = conduit_signal_conformance::pico_local_advertisement()
        .capabilities
        .into_iter()
        .find(|offer| offer.kind_id != template.kind_id)
        .unwrap();
    host.capabilities = (0..count)
        .map(|index| {
            let mut offer = if index == 0 {
                template.clone()
            } else {
                unrelated.clone()
            };
            offer.capability_id = format!("inventory/{index:03}").into();
            offer
        })
        .collect();
    if duplicate && count > 1 {
        host.capabilities[count - 1].capability_id = host.capabilities[0].capability_id.clone();
    }
    if reverse {
        host.capabilities.reverse();
    }
    let hosts = [host];
    let clock = MonotonicClockIdentity::new(
        hosts[0].host_id.clone(),
        hosts[0].boot_id.clone(),
        "inventory/clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let now = MonotonicInstant::new(105, clock.clone()).unwrap();
    let window = CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(110, clock).unwrap(),
    )
    .unwrap();
    let observations = common::resource_observations(&hosts);
    let timed = observations
        .iter()
        .map(|observation| TimedResourceObservation::admit(observation, window.clone()).unwrap())
        .collect::<Vec<_>>();
    let contexts = [CostEvaluationContext {
        host: &hosts[0],
        calibrations: &[],
        now: &now,
        execution_observations: &[],
    }];
    let gear = common::pulse_gear();
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    select_realization_with_costs(
        demand,
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &HardRealizationRequirements::default(),
            observations: &timed,
            tie_policy: &RealizationPolicy::default(),
            forecasts: &[],
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::IgnoreCostsIfIncomplete,
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .map(|_| ())
}
#[test]
fn maximum_inventory_accepts_unique_ids_in_either_order() {
    let result = select(MAXIMUM_COST_OFFERS_PER_HOST, false, false);
    assert!(result.is_ok(), "{result:?}");
    assert!(select(MAXIMUM_COST_OFFERS_PER_HOST, false, true).is_ok());
}
#[test]
fn duplicate_at_opposite_inventory_ends_refuses() {
    for reverse in [false, true] {
        assert!(matches!(
            select(MAXIMUM_COST_OFFERS_PER_HOST, true, reverse),
            Err(CostSelectionError::Evidence(
                CostEvidenceRefusal::AmbiguousEvidence
            ))
        ));
    }
}
#[test]
fn over_bound_inventory_refuses_before_selection() {
    assert!(matches!(
        select(MAXIMUM_COST_OFFERS_PER_HOST + 1, false, false),
        Err(CostSelectionError::Evidence(
            CostEvidenceRefusal::InputLimit
        ))
    ));
}
