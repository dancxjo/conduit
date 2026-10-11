//! No eligible implementation remains a truthful bounded historical account.
use crate::*;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

#[test]
fn all_hard_rejections_from_the_ordinary_selector_remain_explainable() {
    let gear = conduit_plot::parse_with_startup(
        "plot rejected {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap()
    .gears
    .remove(0);
    let mut host = conduit_signal_conformance::pico_local_advertisement();
    host.capabilities
        .retain(|offer| gear.accepts_realization(offer));
    let hosts = [host];
    let now = MonotonicInstant::new(
        105,
        MonotonicClockIdentity::new(
            hosts[0].host_id.clone(),
            hosts[0].boot_id.clone(),
            "cost-clock".into(),
            TemporalScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let contexts = [CostEvaluationContext {
        host: &hosts[0],
        calibrations: &[],
        now: &now,
    }];
    let requirements = HardRealizationRequirements {
        minimum_queue_items: u16::MAX,
        ..Default::default()
    };
    let policy = RealizationPolicy::default();
    let result = select_realization_with_costs(
        CostDemand::admit(&gear, &[]).unwrap(),
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &[],
            tie_policy: &policy,
            forecasts: &[],
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::PreferKnown,
        },
        &mut ExactNumericWork::new(0),
    );
    let error = result.unwrap_err();
    let text = render_cost_selection_refusal(&error).unwrap();
    assert!(text.contains("no selected candidate; costs not compared"));
    assert!(text.contains("Rejected(QueueItemBound)"));
    assert!(text.contains(hosts[0].boot_id.as_str()));
    assert!(!text.contains("effective_fulfillment_seconds"));
    let CostSelectionError::HardAdmission(mut refusal) = error else {
        panic!("retained hard refusal");
    };
    refusal.signs[0].disposition = RealizationDecisionDisposition::Selected;
    assert_eq!(
        render_cost_selection_refusal(&CostSelectionError::HardAdmission(refusal)),
        Err(CostExplanationRefusal::InconsistentSelection)
    );
}
