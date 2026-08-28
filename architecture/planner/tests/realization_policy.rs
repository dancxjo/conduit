mod common;

use common::{competing_hosts, pulse_gear};
use conduit_core::{ResourceClassId, TIMER_RESOURCE_CLASS};
use conduit_planner::{
    select_realization_with_policy, HardRealizationRequirements, RealizationPolicy,
    RealizationPreference,
};

#[test]
fn different_ordered_policies_choose_different_equal_face_realizations() {
    let gear = pulse_gear();
    let hosts = competing_hosts();
    assert_eq!(
        hosts[0].capabilities[0].checked_face(),
        hosts[1].capabilities[0].checked_face(),
        "nominal identity does not alter functional compatibility"
    );

    let efficient = select_realization_with_policy(
        &gear,
        &hosts,
        &HardRealizationRequirements::default(),
        &RealizationPolicy {
            preferences: vec![
                RealizationPreference::MinimizeResourceUnits(ResourceClassId::from(
                    TIMER_RESOURCE_CLASS,
                )),
                RealizationPreference::MaximizeQueueItems,
            ],
        },
    )
    .expect("resource policy selects");
    assert_eq!(efficient.host_id.as_str(), "host-a-efficient");

    let capable = select_realization_with_policy(
        &gear,
        &hosts,
        &HardRealizationRequirements::default(),
        &RealizationPolicy {
            preferences: vec![
                RealizationPreference::MaximizeQueueItems,
                RealizationPreference::MinimizeResourceUnits(ResourceClassId::from(
                    TIMER_RESOURCE_CLASS,
                )),
            ],
        },
    )
    .expect("capacity policy selects");
    assert_eq!(capable.host_id.as_str(), "host-b-capable");
}

#[test]
fn hard_inadmissibility_prevents_a_policy_favorite_from_winning() {
    let gear = pulse_gear();
    let hosts = competing_hosts();
    let selected = select_realization_with_policy(
        &gear,
        &hosts,
        &HardRealizationRequirements {
            minimum_queue_items: 8,
            ..HardRealizationRequirements::default()
        },
        &RealizationPolicy {
            preferences: vec![RealizationPreference::MinimizeResourceUnits(
                ResourceClassId::from(TIMER_RESOURCE_CLASS),
            )],
        },
    )
    .expect("one realization remains hard-admissible");
    assert_eq!(selected.host_id.as_str(), "host-b-capable");
}

#[test]
fn identical_inputs_are_deterministic_independent_of_hosts_order() {
    let gear = pulse_gear();
    let [first, second] = competing_hosts();
    let policy = RealizationPolicy::default();
    let requirements = HardRealizationRequirements::default();
    let forward = select_realization_with_policy(
        &gear,
        &[first.clone(), second.clone()],
        &requirements,
        &policy,
    )
    .expect("forward selection");
    let reversed = select_realization_with_policy(&gear, &[second, first], &requirements, &policy)
        .expect("reverse selection");
    assert_eq!(forward, reversed);
    assert_eq!(forward.host_id.as_str(), "host-a-efficient");
}
