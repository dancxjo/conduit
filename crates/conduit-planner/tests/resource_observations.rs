mod common;

use common::{competing_hosts, pulse_operation};
use conduit_core::{EvidenceId, ResourceHealth, ResourceObservation};
use conduit_planner::{
    select_realization_with_observations, HardRealizationRequirements, PlannerError,
    RealizationPolicy, RealizationPreference,
};

fn observation(
    host: &conduit_core::HostAdvertisement,
    health: ResourceHealth,
    unreserved_units: u32,
    utilized_units: u32,
    evidence: &str,
) -> ResourceObservation {
    let required_class = &host.capabilities[0].resource_requirements[0].class_id;
    let pool = host
        .resources
        .iter()
        .find(|pool| &pool.class_id == required_class)
        .expect("host advertises its required pool");
    ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: pool.pool_id.clone(),
        class_id: pool.class_id.clone(),
        health,
        unreserved_units,
        utilized_units,
        evidence_id: EvidenceId::from(evidence),
    }
}

#[test]
fn changing_observations_change_selection_without_mutating_stable_offers() {
    let operation = pulse_operation();
    let realm = competing_hosts();
    let stable_realm = realm.clone();
    let policy = RealizationPolicy {
        preferences: vec![RealizationPreference::MinimizeResourceUnits(
            realm[0].capabilities[0].resource_requirements[0]
                .class_id
                .clone(),
        )],
    };
    let requirements = HardRealizationRequirements::default();

    let efficient_unavailable = vec![
        observation(
            &realm[0],
            ResourceHealth::Unavailable,
            0,
            0,
            "efficient-down",
        ),
        observation(&realm[1], ResourceHealth::Ready, 3, 1, "capable-ready"),
    ];
    let selected = select_realization_with_observations(
        &operation,
        &realm,
        &requirements,
        &efficient_unavailable,
        &policy,
    )
    .expect("currently ready capable host is selected");
    assert_eq!(selected.host_id.as_str(), "host-b-capable");

    let efficient_ready = vec![
        observation(&realm[0], ResourceHealth::Ready, 1, 3, "efficient-ready"),
        observation(&realm[1], ResourceHealth::Ready, 3, 1, "capable-ready"),
    ];
    let selected = select_realization_with_observations(
        &operation,
        &realm,
        &requirements,
        &efficient_ready,
        &policy,
    )
    .expect("policy can select the newly ready efficient host");
    assert_eq!(selected.host_id.as_str(), "host-a-efficient");
    assert_eq!(
        realm, stable_realm,
        "observations never mutate stable offers"
    );
}

#[test]
fn utilization_is_distinct_from_unreserved_planning_capacity() {
    let operation = pulse_operation();
    let realm = competing_hosts();
    let observations = [
        observation(
            &realm[0],
            ResourceHealth::Ready,
            1,
            3,
            "busy-but-admissible",
        ),
        observation(&realm[1], ResourceHealth::Unavailable, 0, 0, "capable-down"),
    ];
    let selected = select_realization_with_observations(
        &operation,
        &realm,
        &HardRealizationRequirements::default(),
        &observations,
        &RealizationPolicy::default(),
    )
    .expect("one unreserved unit admits the one-unit realization");
    assert_eq!(selected.host_id.as_str(), "host-a-efficient");
}

#[test]
fn stale_or_incoherent_observations_fail_closed() {
    let operation = pulse_operation();
    let realm = competing_hosts();
    let mut stale = observation(&realm[0], ResourceHealth::Ready, 1, 0, "stale");
    stale.boot_id = conduit_core::BootId::from("old-boot");
    assert!(matches!(
        select_realization_with_observations(
            &operation,
            &realm,
            &HardRealizationRequirements::default(),
            &[stale],
            &RealizationPolicy::default(),
        ),
        Err(PlannerError::InvalidResourceObservation(_))
    ));

    let incoherent = observation(&realm[0], ResourceHealth::Unavailable, 1, 0, "incoherent");
    assert!(matches!(
        select_realization_with_observations(
            &operation,
            &realm,
            &HardRealizationRequirements::default(),
            &[incoherent],
            &RealizationPolicy::default(),
        ),
        Err(PlannerError::InvalidResourceObservation(_))
    ));
}
