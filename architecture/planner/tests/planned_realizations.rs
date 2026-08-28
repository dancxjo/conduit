mod common;

use common::competing_hosts;
use conduit_core::{ResourceHealth, ResourceObservation, SignId};
use conduit_form::parse_with_startup;
use conduit_planner::{plan_selected_realizations, RealizationPolicy, RealizationPreference};
use conduit_signal::signal_profile_catalog;
use std::collections::BTreeMap;

fn two_pulse_form() -> conduit_form::CheckedForm {
    parse_with_startup(
        "form realization {\n    first: flow/pulse(count = 2, period-ms = 0, initial = false)\n    second: flow/pulse(count = 2, period-ms = 0, initial = false)\n\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .expect("two equal-face gears check")
}

fn observations(hosts: &[conduit_core::HostAdvertisement; 2]) -> Vec<ResourceObservation> {
    hosts
        .iter()
        .enumerate()
        .map(|(index, host)| {
            let pool = host
                .resources
                .iter()
                .find(|pool| {
                    pool.class_id == host.capabilities[0].resource_requirements[0].class_id
                })
                .expect("required pool exists");
            ResourceObservation {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                pool_id: pool.pool_id.clone(),
                class_id: pool.class_id.clone(),
                health: ResourceHealth::Ready,
                unreserved_units: if index == 0 { 1 } else { 3 },
                utilized_units: 0,
                sign_id: SignId::from(format!("observation-{index}")),
            }
        })
        .collect()
}

#[test]
fn whole_form_selection_shares_observed_capacity_and_seals_exact_realizations() {
    let form = two_pulse_form();
    let hosts = competing_hosts();
    let resource_class = hosts[0].capabilities[0].resource_requirements[0]
        .class_id
        .clone();
    let policies = form
        .gears
        .iter()
        .map(|gear| {
            (
                gear.gear_id.clone(),
                RealizationPolicy {
                    preferences: vec![RealizationPreference::MinimizeResourceUnits(
                        resource_class.clone(),
                    )],
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let plan = plan_selected_realizations(
        &form,
        &hosts,
        &[],
        &BTreeMap::new(),
        &observations(&hosts),
        &policies,
    )
    .expect("remaining observed capacity selects one realization per host");
    let mut planned = plan
        .fragments
        .iter()
        .flat_map(|fragment| fragment.placements.iter())
        .collect::<Vec<_>>();
    planned.sort_by(|left, right| left.gear_id.cmp(&right.gear_id));

    assert_eq!(planned[0].host_id.as_str(), "host-a-efficient");
    assert_eq!(planned[1].host_id.as_str(), "host-b-capable");
    for gear in planned {
        let host = hosts
            .iter()
            .find(|host| host.host_id == gear.host_id)
            .expect("planned host exists");
        let offer = &host.capabilities[0];
        assert_eq!(gear.capability_id, offer.capability_id);
        assert_eq!(
            gear.implementation_id,
            offer.implementation.implementation_id
        );
        assert_eq!(gear.artifact_id, offer.implementation.artifact_id);
        assert_eq!(gear.limits, offer.limits);
        assert_eq!(
            gear.resources[0].units,
            offer.resource_requirements[0].units
        );
    }
}

#[test]
fn selected_semantic_limits_participate_in_plan_identity() {
    let form = two_pulse_form();
    let hosts = competing_hosts();
    let original = plan_selected_realizations(
        &form,
        &hosts,
        &[],
        &BTreeMap::new(),
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("original plan seals");

    let mut changed_hosts = hosts.clone();
    changed_hosts[1].capabilities[0].limits.max_queue_bytes += 1;
    let changed = plan_selected_realizations(
        &form,
        &changed_hosts,
        &[],
        &BTreeMap::new(),
        &observations(&changed_hosts),
        &BTreeMap::new(),
    )
    .expect("changed finite limits seal");
    assert_ne!(original.plan_id, changed.plan_id);
}
