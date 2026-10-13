//! Ordinary Plan resource binding and minimum admission; metadata is descriptive,
//! not authenticated provider possession or physical compute execution evidence.
use super::*;
use crate::{plan, PlacementChoice, PlacementChoices};
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;

fn owner(
    capability: &CapabilityOffer,
    pool: &ResourcePoolId,
    suffix: &str,
) -> BaseProviderAdvertisement {
    BaseProviderAdvertisement {
        base_id: alloc::format!("fixture/base/{suffix}").into(),
        provider_instance_id: alloc::format!("fixture/provider/{suffix}").into(),
        provider_generation: 1,
        implementation_id: "fixture/base@1".into(),
        mechanism_family: "fixture/timer".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: vec![capability.capability_id.clone()],
        resource_pool_ids: vec![pool.clone()],
    }
}
fn fixture() -> (
    conduit_plot::CheckedPlot,
    HostAdvertisement,
    PlacementChoices,
) {
    let plot = conduit_plot::parse_with_startup(
        "plot owned {\n first: flow/pulse(count = 1, period-ms = 0, initial = false)\n second: flow/pulse(count = 1, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(), &conduit_signal::signal_profile_catalog(),
    ).unwrap();
    let mut host = crate::tests::host();
    let mut second = host.capabilities[0].clone();
    second.capability_id = "pulse-second".into();
    host.capabilities = vec![host.capabilities[0].clone(), second];
    let class = &host.capabilities[0].resource_requirements[0].class_id;
    let original = host
        .resources
        .iter()
        .find(|resource| &resource.class_id == class)
        .unwrap();
    let mut first = original.clone();
    first.pool_id = "fixture/pool/first".into();
    first.capacity_units = host.capabilities[0].resource_requirements[0].units;
    let mut second = first.clone();
    second.pool_id = "fixture/pool/second".into();
    host.resources = vec![first, second];
    host.bases = vec![
        owner(&host.capabilities[0], &host.resources[0].pool_id, "first"),
        owner(&host.capabilities[1], &host.resources[1].pool_id, "second"),
    ];
    let choices = PlacementChoices {
        by_gear: plot
            .gears
            .iter()
            .zip(&host.capabilities)
            .map(|(gear, cap)| {
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: cap.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    (plot, host, choices)
}
#[test]
fn two_same_class_base_owners_bind_each_selected_capability_to_its_own_pool() {
    let (plot, host, choices) = fixture();
    let prepared = plan(&plot, core::slice::from_ref(&host), &choices, &[]).unwrap();
    let placements = &prepared.fragments[0].placements;
    for (index, gear) in plot.gears.iter().enumerate() {
        let selected = placements
            .iter()
            .find(|placement| placement.gear_id == gear.gear_id)
            .unwrap();
        assert_eq!(selected.resources[0].pool_id, host.resources[index].pool_id);
        assert_eq!(
            selected.base.as_ref().unwrap().provider_instance_id,
            host.bases[index].provider_instance_id
        );
    }
    assert!(verify_plan(&prepared));
}
#[test]
fn owned_capacity_is_independent_and_never_borrows_another_base_pool() {
    let (plot, mut host, mut choices) = fixture();
    host.resources[1].capacity_units = 100;
    choices
        .by_gear
        .get_mut(&plot.gears[1].gear_id)
        .unwrap()
        .capability_id = host.capabilities[0].capability_id.clone();
    assert!(matches!(
        plan(&plot, &[host], &choices, &[]),
        Err(crate::PlannerError::ResourceCapacityExceeded(_))
    ));
}
#[test]
fn missing_foreign_class_multiple_owned_and_ambiguous_base_provenance_refuse() {
    for fault in 0..6 {
        let (plot, mut host, choices) = fixture();
        match fault {
            0 => host.bases[0].resource_pool_ids = vec!["missing/pool".into()],
            1 => {
                host.resources[1].class_id = "foreign/class".into();
                host.bases[0].resource_pool_ids = vec![host.resources[1].pool_id.clone()];
                host.bases[1]
                    .resource_pool_ids
                    .push(host.resources[0].pool_id.clone());
            }
            2 => host.bases[0]
                .resource_pool_ids
                .push(host.resources[1].pool_id.clone()),
            3 => host.bases[1]
                .capability_ids
                .push(host.capabilities[0].capability_id.clone()),
            4 => host.bases[0].lifecycle = BaseLifecycle::Lost,
            _ => host.bases[1]
                .resource_pool_ids
                .push(host.resources[0].pool_id.clone()),
        }
        assert!(
            plan(&plot, &[host], &choices, &[]).is_err(),
            "fault {fault}"
        );
    }
}
#[test]
fn base_free_profile_keeps_unique_class_selection_and_refuses_ambiguity() {
    let (_, mut host, _) = fixture();
    host.bases.clear();
    let class = host.capabilities[0].resource_requirements[0]
        .class_id
        .clone();
    assert!(resolve_resource_pool(&host, &host.capabilities[0], &class).is_err());
    host.resources.pop();
    assert_eq!(
        resolve_resource_pool(&host, &host.capabilities[0], &class)
            .unwrap()
            .pool_id,
        host.resources[0].pool_id
    );
}
#[test]
fn compute_minima_use_the_same_exact_selected_base_pool_resolution() {
    let (plot, mut host, choices) = fixture();
    let fixture = conduit_ai::llm_generate_base_fixtures()[0].clone();
    let requirement = fixture.advertisement.capabilities[0]
        .resource_requirements
        .iter()
        .find(|requirement| requirement.compute.is_some())
        .unwrap()
        .clone();
    let pool = fixture
        .advertisement
        .resources
        .iter()
        .find(|pool| pool.class_id == requirement.class_id)
        .unwrap()
        .clone();
    for index in 0..2 {
        host.capabilities[index].resource_requirements = vec![requirement.clone()];
        host.resources[index] = pool.clone();
        host.resources[index].pool_id = host.bases[index].resource_pool_ids[0].clone();
        host.resources[index].capacity_units = requirement.units;
    }
    let hosts = BTreeMap::from([(host.host_id.clone(), &host)]);
    let minima = crate::compute_admission::admit_minima(&plot.gears, &hosts, &choices).unwrap();
    for pool in &host.resources {
        assert_eq!(
            minima[&(host.host_id.clone(), pool.pool_id.clone())],
            requirement.units
        );
    }
    host.bases[0].resource_pool_ids.clear();
    let hosts = BTreeMap::from([(host.host_id.clone(), &host)]);
    let shared = crate::compute_admission::admit_minima(&plot.gears, &hosts, &choices).unwrap();
    assert_eq!(
        shared[&(host.host_id.clone(), host.resources[0].pool_id.clone())],
        requirement.units
    );
    host.bases[1]
        .resource_pool_ids
        .push(host.resources[0].pool_id.clone());
    let hosts = BTreeMap::from([(host.host_id.clone(), &host)]);
    assert!(crate::compute_admission::admit_minima(&plot.gears, &hosts, &choices).is_err());
}

#[test]
fn capability_base_with_no_owned_matching_pool_can_use_unique_host_shared_pool() {
    let (plot, mut host, choices) = fixture();
    host.bases[0].resource_pool_ids.clear();
    let prepared = plan(&plot, core::slice::from_ref(&host), &choices, &[]).unwrap();
    let first = prepared.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.gear_id == plot.gears[0].gear_id)
        .unwrap();
    assert_eq!(first.resources[0].pool_id, host.resources[0].pool_id);
    assert_eq!(
        first.base.as_ref().unwrap().provider_instance_id,
        host.bases[0].provider_instance_id
    );
    // A different class physically owned by this Base does not prohibit using
    // a separately Host-shared timer pool.
    let mut other = host.resources[0].clone();
    other.pool_id = "fixture/owned-other-class".into();
    other.class_id = "fixture/other-class".into();
    host.bases[0].resource_pool_ids.push(other.pool_id.clone());
    host.resources.push(other);
    host.resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    plan(&plot, &[host], &choices, &[]).unwrap();
}
#[test]
fn shared_host_fallback_refuses_foreign_only_and_multiple_unowned_class_pools() {
    let (_, mut host, _) = fixture();
    let class = host.capabilities[0].resource_requirements[0]
        .class_id
        .clone();
    host.bases[0].resource_pool_ids.clear();
    let mut shared = host.resources[0].clone();
    shared.pool_id = "fixture/second-unowned".into();
    host.resources.push(shared);
    assert!(resolve_resource_pool(&host, &host.capabilities[0], &class).is_err());
    host.resources.pop();
    host.bases[1]
        .resource_pool_ids
        .push(host.resources[0].pool_id.clone());
    assert!(resolve_resource_pool(&host, &host.capabilities[0], &class).is_err());
}
#[test]
fn exhausted_owned_pool_never_falls_back_to_large_host_shared_pool() {
    let (plot, mut host, mut choices) = fixture();
    let mut shared = host.resources[0].clone();
    shared.pool_id = "fixture/unowned-large".into();
    shared.capacity_units = 100;
    host.resources.push(shared);
    choices
        .by_gear
        .get_mut(&plot.gears[1].gear_id)
        .unwrap()
        .capability_id = host.capabilities[0].capability_id.clone();
    assert!(matches!(
        plan(&plot, &[host], &choices, &[]),
        Err(crate::PlannerError::ResourceCapacityExceeded(_))
    ));
}
