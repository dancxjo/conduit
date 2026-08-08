use conduit_core::{
    ArtifactId, CapabilityId, HostId, ImplementationId, KindContractRevision, ResourceClassId,
    TIMER_RESOURCE_CLASS,
};
use conduit_form::parse;
use conduit_planner::{
    select_realization_with_policy, HardRealizationRequirements, RealizationPolicy,
    RealizationPreference,
};
use conduit_signal::{pico_local_advertisement, signal_profile_catalog, PULSE_KIND};

fn pulse_operation() -> conduit_form::CheckedOperation {
    parse(
        "form 0\n\npolicy {\n    pulse: flow/pulse\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n}\n",
        &signal_profile_catalog(),
    )
    .expect("pulse form checks")
    .operations
    .remove(0)
}

fn competing_hosts() -> [conduit_core::HostAdvertisement; 2] {
    let source = pico_local_advertisement();
    let pulse = source
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == PULSE_KIND)
        .expect("pulse offer exists");

    let mut efficient_offer = pulse.clone();
    efficient_offer.capability_id = CapabilityId::from("efficient/pulse");
    efficient_offer.implementation.implementation_id = ImplementationId::from("efficient/pulse@1");
    efficient_offer.implementation.artifact_id = ArtifactId::from("efficient/pulse-artifact@1");
    efficient_offer.limits.max_queue_items = 4;
    efficient_offer.resource_requirements[0].units = 1;

    let mut capable_offer = pulse.clone();
    capable_offer.capability_id = CapabilityId::from("capable/pulse");
    capable_offer.kind_id = conduit_core::kind_id("alternate/nominal-pulse");
    capable_offer.kind_contract_revision = KindContractRevision::from("alternate/nominal-pulse@9");
    capable_offer.implementation.implementation_id = ImplementationId::from("capable/pulse@1");
    capable_offer.implementation.artifact_id = ArtifactId::from("capable/pulse-artifact@1");
    capable_offer.limits.max_queue_items = 8;
    capable_offer.resource_requirements[0].units = 3;

    let mut efficient = source.clone();
    efficient.host_id = HostId::from("host-a-efficient");
    efficient.capabilities = vec![efficient_offer];
    let mut capable = source;
    capable.host_id = HostId::from("host-b-capable");
    capable.capabilities = vec![capable_offer];
    [efficient, capable]
}

#[test]
fn different_ordered_policies_choose_different_equal_face_realizations() {
    let operation = pulse_operation();
    let realm = competing_hosts();
    assert_eq!(
        realm[0].capabilities[0].checked_face(),
        realm[1].capabilities[0].checked_face(),
        "nominal identity does not alter functional compatibility"
    );

    let efficient = select_realization_with_policy(
        &operation,
        &realm,
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
        &operation,
        &realm,
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
    let operation = pulse_operation();
    let realm = competing_hosts();
    let selected = select_realization_with_policy(
        &operation,
        &realm,
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
fn identical_inputs_are_deterministic_independent_of_realm_order() {
    let operation = pulse_operation();
    let [first, second] = competing_hosts();
    let policy = RealizationPolicy::default();
    let requirements = HardRealizationRequirements::default();
    let forward = select_realization_with_policy(
        &operation,
        &[first.clone(), second.clone()],
        &requirements,
        &policy,
    )
    .expect("forward selection");
    let reversed =
        select_realization_with_policy(&operation, &[second, first], &requirements, &policy)
            .expect("reverse selection");
    assert_eq!(forward, reversed);
    assert_eq!(forward.host_id.as_str(), "host-a-efficient");
}
