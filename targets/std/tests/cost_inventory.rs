//! Real owner inventories and separately bounded cost candidates.
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_std_host::{StdHost, StdHostConfig};
fn reference_hosts() -> Vec<HostAdvertisement> {
    (0..2)
        .map(|i| {
            StdHost::new_with_config(StdHostConfig {
                host_id: format!("cost-inventory/host-{i}").into(),
                boot_id: format!("cost-inventory/boot-{i}").into(),
                offer_generation: OfferGeneration(1),
            })
            .advertisement()
            .clone()
        })
        .collect()
}
fn exercise(
    hosts: &[HostAdvertisement],
    requirements: HardRealizationRequirements,
    intent: PerformanceIntent,
    inspect: impl FnOnce(Result<CostRealizationSelection<'_>, CostSelectionError>),
) {
    let gear = conduit_plot::parse_with_startup(
        "plot inventory {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap()
    .gears
    .remove(0);
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let identity = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "inventory-fixture/clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap();
            CostObservationWindow::admit(
                MonotonicInstant::new(100, identity.clone()).unwrap(),
                MonotonicInstant::new(110, identity).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let clocks: Vec<_> = windows
        .iter()
        .map(|window| MonotonicInstant::new(105, window.observed().clock().clone()).unwrap())
        .collect();
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&clocks)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &[],
            now,
            execution_observations: &[],
        })
        .collect();
    // Controlled current-capacity facts reference the actual owner inventories.
    // They are fixture evidence, not measurements of this machine's load.
    let observations: Vec<_> = hosts
        .iter()
        .flat_map(|host| {
            host.resources.iter().map(|pool| ResourceObservation {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                pool_id: pool.pool_id.clone(),
                class_id: pool.class_id.clone(),
                health: ResourceHealth::Ready,
                unreserved_units: pool.capacity_units,
                utilized_units: 0,
                sign_id: format!(
                    "inventory-fixture/{}/{}",
                    host.host_id.as_str(),
                    pool.pool_id.as_str()
                )
                .into(),
            })
        })
        .collect();
    let timed: Vec<_> = observations
        .iter()
        .map(|observation| {
            let index = hosts
                .iter()
                .position(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, windows[index].clone()).unwrap()
        })
        .collect();

    let tie_policy = RealizationPolicy::default();
    let result = select_realization_with_costs(
        CostDemand::admit(&gear, &[]).unwrap(),
        CostSelectionInputs {
            hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &timed,
            tie_policy: &tie_policy,
            forecasts: &[],
            intent,
            unavailable: UnavailableCostPolicy::IgnoreCostsIfIncomplete,
        },
        &mut ExactNumericWork::new(0),
    );
    inspect(result);
}
#[test]
fn unrelated_std_offers_do_not_exhaust_a_two_candidate_cost_decision() {
    let hosts = reference_hosts();
    let counts: Vec<_> = hosts.iter().map(|host| host.capabilities.len()).collect();
    assert_eq!(counts.len(), 2);
    assert!(counts.iter().all(|count| *count > MAXIMUM_COST_CANDIDATES));
    exercise(
        &hosts,
        HardRealizationRequirements::default(),
        PerformanceIntent::Interactive,
        |result| {
            let selection = result.unwrap_or_else(|refusal| {
                panic!("actual Offer inventories {counts:?} refused: {refusal:?}")
            });
            assert_eq!(selection.costs.len(), 2);
            assert!(!selection.used_cost_preference);
            assert_eq!(selection.arithmetic_work_spent, 0);
            assert!(hosts
                .iter()
                .any(|host| host.host_id == selection.realization.choice.host_id));
        },
    );
}
/// Controlled alternative declarations share the stock checked pulse Back.
/// This boundary fixture does not claim those extra Offers were owner-published.
fn alternatives(count: usize, rejected: usize) -> Vec<HostAdvertisement> {
    let mut hosts = reference_hosts();
    hosts.truncate(1);
    let pulse = hosts[0]
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_signal::PULSE_KIND)
        .unwrap()
        .clone();
    hosts[0]
        .capabilities
        .retain(|offer| offer.kind_id != pulse.kind_id);
    for i in 0..count {
        let mut offer = pulse.clone();
        offer.capability_id = format!("inventory-boundary/pulse-{i}").into();
        offer.limits.max_queue_items = if i < rejected { 0 } else { 1 };
        hosts[0].capabilities.push(offer);
    }
    hosts
}
#[test]
fn thirty_two_cost_candidates_pass_and_thirty_three_retain_the_limit_refusal() {
    for count in [32, 33] {
        let hosts = alternatives(count, 0);
        exercise(
            &hosts,
            HardRealizationRequirements::default(),
            PerformanceIntent::Interactive,
            |result| {
                if count == 32 {
                    let selection = result.unwrap();
                    assert_eq!(selection.costs.len(), 32);
                    assert_eq!(selection.arithmetic_work_spent, 0);
                } else {
                    let error = result.unwrap_err();
                    let CostSelectionError::Policy(account) = &error else {
                        panic!("{error:?}")
                    };
                    assert_eq!(account.reason, CostEvidenceRefusal::InputLimit);
                    assert_eq!(account.realization.signs.len(), 33);
                    let rendered =
                        conduit_observatory::render_cost_selection_refusal(&error).unwrap();
                    assert!(rendered.contains("admitted_candidates=33"));
                    assert!(rendered.contains("no cost-admitted winner"));
                    assert!(!rendered.contains("hard_admission=Selected"));
                    let CostSelectionError::Policy(mut account) = error else {
                        unreachable!()
                    };
                    account.reason = CostEvidenceRefusal::UnsupportedIntent;
                    account.intent = PerformanceIntent::BoundedResponse;
                    assert_eq!(
                        conduit_observatory::render_cost_selection_refusal(
                            &CostSelectionError::Policy(account)
                        ),
                        Err(conduit_observatory::CostExplanationRefusal::InconsistentSelection)
                    );
                }
            },
        );
    }
}
#[test]
fn more_than_thirty_two_hard_records_remain_explainable_with_fewer_cost_candidates() {
    for rejected in [31, 35] {
        let hosts = alternatives(35, rejected);
        let requirements = HardRealizationRequirements {
            minimum_queue_items: 1,
            ..Default::default()
        };
        exercise(
            &hosts,
            requirements,
            PerformanceIntent::Interactive,
            |result| {
                let rendered = if rejected == 35 {
                    let error = result.unwrap_err();
                    let CostSelectionError::HardAdmission(account) = &error else {
                        panic!("{error:?}")
                    };
                    assert_eq!(account.signs.len(), 35);
                    conduit_observatory::render_cost_selection_refusal(&error).unwrap()
                } else {
                    let selection = result.unwrap();
                    assert_eq!(selection.costs.len(), 4);
                    assert_eq!(selection.realization.signs.len(), 35);
                    conduit_observatory::render_cost_selection_explanation(&selection).unwrap()
                };
                assert_eq!(rendered.matches("QueueItemBound").count(), rejected);
            },
        );
    }
}
#[test]
fn full_inventory_bound_is_independent_and_unsupported_intent_retains_hard_evidence() {
    let mut hosts = reference_hosts();
    let original = hosts[0].capabilities[0].clone();
    while hosts[0].capabilities.len() <= MAXIMUM_COST_OFFERS_PER_HOST {
        let mut offer = original.clone();
        offer.capability_id = format!("inventory-limit/{}", hosts[0].capabilities.len()).into();
        hosts[0].capabilities.push(offer);
    }
    exercise(
        &hosts,
        HardRealizationRequirements::default(),
        PerformanceIntent::Interactive,
        |result| {
            assert!(matches!(
                result,
                Err(CostSelectionError::Evidence(
                    CostEvidenceRefusal::InputLimit
                ))
            ))
        },
    );
    let hosts = reference_hosts();
    exercise(
        &hosts,
        HardRealizationRequirements::default(),
        PerformanceIntent::BoundedResponse,
        |result| {
            let mut error = result.unwrap_err();
            let rendered = conduit_observatory::render_cost_selection_refusal(&error).unwrap();
            assert!(rendered.contains("UnsupportedIntent"));
            assert!(!rendered.contains("hard_admission=Selected"));
            let CostSelectionError::Policy(account) = &mut error else {
                panic!("{error:?}")
            };
            account.reason = CostEvidenceRefusal::Stale;
            assert_eq!(
                conduit_observatory::render_cost_selection_refusal(&error),
                Err(conduit_observatory::CostExplanationRefusal::InconsistentSelection)
            );
        },
    );
}
