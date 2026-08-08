mod common;

use common::competing_hosts;
use conduit_core::{EvidenceId, ResourceHealth, ResourceObservation};
use conduit_form::parse;
use conduit_planner::{plan_selected_realizations, RealizationPolicy, RealizationPreference};
use conduit_signal::signal_profile_catalog;
use std::collections::BTreeMap;

fn two_pulse_form() -> conduit_form::CheckedForm {
    parse(
        "form 0\n\nrealization {\n    first: flow/pulse\n    second: flow/pulse\n\n    first.count = 2\n    first.period-ms = 0\n    first.initial = false\n    second.count = 2\n    second.period-ms = 0\n    second.initial = false\n}\n",
        &signal_profile_catalog(),
    )
    .expect("two equal-face operations check")
}

fn observations(realm: &[conduit_core::HostAdvertisement; 2]) -> Vec<ResourceObservation> {
    realm
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
                evidence_id: EvidenceId::from(format!("observation-{index}")),
            }
        })
        .collect()
}

#[test]
fn whole_form_selection_shares_observed_capacity_and_seals_exact_realizations() {
    let form = two_pulse_form();
    let realm = competing_hosts();
    let resource_class = realm[0].capabilities[0].resource_requirements[0]
        .class_id
        .clone();
    let policies = form
        .operations
        .iter()
        .map(|operation| {
            (
                operation.operation_id.clone(),
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
        &realm,
        &[],
        &BTreeMap::new(),
        &observations(&realm),
        &policies,
    )
    .expect("remaining observed capacity selects one realization per host");
    let mut planned = plan
        .fragments
        .iter()
        .flat_map(|fragment| fragment.placements.iter())
        .collect::<Vec<_>>();
    planned.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));

    assert_eq!(planned[0].host_id.as_str(), "host-a-efficient");
    assert_eq!(planned[1].host_id.as_str(), "host-b-capable");
    for operation in planned {
        let host = realm
            .iter()
            .find(|host| host.host_id == operation.host_id)
            .expect("planned host exists");
        let offer = &host.capabilities[0];
        assert_eq!(operation.capability_id, offer.capability_id);
        assert_eq!(
            operation.implementation_id,
            offer.implementation.implementation_id
        );
        assert_eq!(operation.artifact_id, offer.implementation.artifact_id);
        assert_eq!(operation.limits, offer.limits);
        assert_eq!(
            operation.resources[0].units,
            offer.resource_requirements[0].units
        );
    }
}

#[test]
fn selected_semantic_limits_participate_in_plan_identity() {
    let form = two_pulse_form();
    let realm = competing_hosts();
    let original = plan_selected_realizations(
        &form,
        &realm,
        &[],
        &BTreeMap::new(),
        &observations(&realm),
        &BTreeMap::new(),
    )
    .expect("original plan seals");

    let mut changed_realm = realm.clone();
    changed_realm[1].capabilities[0].limits.max_queue_bytes += 1;
    let changed = plan_selected_realizations(
        &form,
        &changed_realm,
        &[],
        &BTreeMap::new(),
        &observations(&changed_realm),
        &BTreeMap::new(),
    )
    .expect("changed finite limits seal");
    assert_ne!(original.plan_id, changed.plan_id);
}
