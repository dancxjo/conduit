use conduit_ai::{
    generate_text_base_fixtures, generate_text_realization_advertisements,
    install_generate_text_catalog, CPU_EXECUTION_RESOURCE,
};
use conduit_core::{
    seal_plan, ActivePlayId, ArchitectureBaseId, ArchitectureBaseKind, BaseExecutionLaneId, ClueId,
    ComputeDomainId, ComputeLaneAssignment, ComputePerformanceClassId, ComputeServiceGuarantee,
    ComputeTopologyGroup, ComputeTopologyGroupId, ComputeTopologyRequirement, PlacementId,
    ResourceClassId, ResourceHealth, ResourceObservation,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics, select_realization_with_policy,
    HardRealizationRequirements, RealizationPolicy, RealizationPreference,
};
use std::collections::BTreeMap;

fn form(source: &str) -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_generate_text_catalog(&mut startup, &mut profile).expect("catalog installs");
    conduit_form::parse(source, &profile).expect("compute fixture form checks")
}

fn observations(hosts: &[conduit_core::HostAdvertisement]) -> Vec<ResourceObservation> {
    hosts
        .iter()
        .flat_map(|host| {
            host.resources
                .iter()
                .enumerate()
                .map(move |(index, pool)| ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    clue_id: ClueId::from(format!("compute-observation-{index}")),
                })
        })
        .collect()
}

#[test]
fn scalable_compute_ranges_share_one_existing_pool_across_operations() {
    let checked =
        form("form 0\n\nanswer {\n first: ai/generate-text\n second: ai/generate-text\n}\n");
    let mut fixture = generate_text_base_fixtures()[0].clone();
    fixture.advertisement.capabilities[0]
        .limits
        .max_active_instances = 2;
    for pool in &mut fixture.advertisement.resources {
        if pool.class_id.as_str() == CPU_EXECUTION_RESOURCE {
            pool.capacity_units = 5;
        } else {
            pool.capacity_units *= 2;
        }
    }
    let mut hosts = vec![fixture.advertisement.clone()];
    let advertisements = generate_text_realization_advertisements(&[fixture]);
    let plan = plan_selected_realizations_with_characteristics(
        &checked,
        &hosts,
        &[],
        &BTreeMap::new(),
        &advertisements,
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("minimum-first finite allocation admits both gears");
    let selected = plan.fragments[0]
        .placements
        .iter()
        .map(|placement| {
            placement
                .resources
                .iter()
                .find(|binding| binding.class_id.as_str() == CPU_EXECUTION_RESOURCE)
                .expect("compute binding exists")
        })
        .collect::<Vec<_>>();
    assert_eq!(selected[0].units, 3);
    assert_eq!(selected[1].units, 2);
    assert_eq!(selected.iter().map(|binding| binding.units).sum::<u32>(), 5);
    assert!(selected.iter().all(|binding| {
        binding
            .compute
            .as_ref()
            .is_some_and(|compute| compute.service_guarantee == ComputeServiceGuarantee::Shared)
    }));

    hosts[0]
        .resources
        .iter_mut()
        .find(|pool| pool.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute pool exists")
        .capacity_units = 4;
    let constrained = hosts[0].capabilities[0]
        .resource_requirements
        .iter_mut()
        .find(|requirement| requirement.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute requirement exists");
    constrained.units = 2;
    constrained
        .compute
        .as_mut()
        .expect("compute range exists")
        .minimum_lanes = 2;
    let minimum_only = plan_selected_realizations_with_characteristics(
        &checked,
        &hosts,
        &[],
        &BTreeMap::new(),
        &advertisements,
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("joint minima remain feasible when neither preference fits");
    let lanes = minimum_only.fragments[0]
        .placements
        .iter()
        .map(|placement| {
            placement
                .resources
                .iter()
                .find(|binding| binding.class_id.as_str() == CPU_EXECUTION_RESOURCE)
                .expect("compute binding exists")
                .units
        })
        .collect::<Vec<_>>();
    assert_eq!(lanes, vec![2, 2]);
}

#[test]
fn topology_service_and_architecture_base_are_exact_plan_facts() {
    let checked = form("form 0\n\nanswer {\n generate: ai/generate-text\n}\n");
    let mut fixture = generate_text_base_fixtures()[0].clone();
    let capability = &mut fixture.advertisement.capabilities[0];
    let requirement = capability
        .resource_requirements
        .iter_mut()
        .find(|requirement| requirement.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute requirement exists");
    let compute = requirement.compute.as_mut().expect("compute range exists");
    compute.minimum_lanes = 2;
    compute.preferred_lanes = 2;
    compute.maximum_lanes = 2;
    compute.minimum_service_guarantee = ComputeServiceGuarantee::Reserved;
    compute.topology = Some(ComputeTopologyRequirement {
        same_numa_domain: true,
        same_cache_domain: true,
        performance_class: Some(ComputePerformanceClassId::from("performance")),
    });
    requirement.units = 2;
    let pool = fixture
        .advertisement
        .resources
        .iter_mut()
        .find(|pool| pool.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute pool exists");
    let contract = pool.compute.as_mut().expect("compute contract exists");
    contract.service_guarantee = ComputeServiceGuarantee::Exclusive;
    contract.architecture_base_id = ArchitectureBaseId::from("rp2040-base@1");
    contract.architecture_base_kind = ArchitectureBaseKind::BareMetal;
    contract.topology_groups = vec![ComputeTopologyGroup {
        group_id: ComputeTopologyGroupId::from("cluster-0"),
        lane_capacity: 2,
        numa_domain: Some(ComputeDomainId::from("memory-0")),
        cache_domain: Some(ComputeDomainId::from("cache-0")),
        performance_class: Some(ComputePerformanceClassId::from("performance")),
    }];
    let hosts = vec![fixture.advertisement.clone()];
    let advertisements = generate_text_realization_advertisements(&[fixture]);
    let plan = plan_selected_realizations_with_characteristics(
        &checked,
        &hosts,
        &[],
        &BTreeMap::new(),
        &advertisements,
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("exact topology and stronger service satisfy the requirement");
    let binding = plan.fragments[0].placements[0]
        .resources
        .iter()
        .find(|binding| binding.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute binding exists");
    let reservation = binding.compute.as_ref().expect("reservation is explicit");
    assert_eq!(reservation.selected_lanes, 2);
    assert_eq!(
        reservation.service_guarantee,
        ComputeServiceGuarantee::Exclusive
    );
    assert_eq!(
        reservation.architecture_base_kind,
        ArchitectureBaseKind::BareMetal
    );
    assert_eq!(
        reservation.topology_group_id.as_ref().map(|id| id.as_str()),
        Some("cluster-0")
    );

    let mut changed_fragment = plan.fragments[0].clone();
    changed_fragment.placements[0]
        .resources
        .iter_mut()
        .find_map(|binding| binding.compute.as_mut())
        .expect("compute reservation exists")
        .architecture_base_id = ArchitectureBaseId::from("different-base@1");
    let changed = seal_plan(checked.identity(), vec![changed_fragment]);
    assert_ne!(plan.plan_id, changed.plan_id);

    let transient = ComputeLaneAssignment {
        architecture_base_id: reservation.architecture_base_id.clone(),
        base_lane_id: BaseExecutionLaneId::from("physical-core-1"),
        active_play_id: ActivePlayId::from("play-1"),
        placement_id: PlacementId::from("generate"),
    };
    assert_eq!(transient.base_lane_id.as_str(), "physical-core-1");
    let encoded_plan = serde_json::to_string(&plan).expect("Plan serializes");
    assert!(!encoded_plan.contains("physical-core-1"));
    assert!(!encoded_plan.contains("base_lane_id"));
}

#[test]
fn shared_service_or_missing_topology_cannot_satisfy_stronger_requirements() {
    let fixture = generate_text_base_fixtures()[0].clone();
    let offer = fixture
        .advertisement
        .resources
        .iter()
        .find(|pool| pool.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute offer exists");
    let mut requirement = fixture.advertisement.capabilities[0]
        .resource_requirements
        .iter()
        .find(|requirement| requirement.class_id.as_str() == CPU_EXECUTION_RESOURCE)
        .expect("compute requirement exists")
        .clone();
    requirement
        .compute
        .as_mut()
        .expect("compute range exists")
        .minimum_service_guarantee = ComputeServiceGuarantee::Exclusive;
    assert!(conduit_core::compute_reservation(&requirement, offer, 3).is_none());
    let compute = requirement.compute.as_mut().expect("compute range exists");
    compute.minimum_service_guarantee = ComputeServiceGuarantee::Shared;
    compute.topology = Some(ComputeTopologyRequirement {
        same_numa_domain: true,
        same_cache_domain: false,
        performance_class: None,
    });
    assert!(conduit_core::compute_reservation(&requirement, offer, 3).is_none());
}

#[test]
fn policy_can_prefer_service_without_conflating_implementation_and_artifact() {
    let checked = form("form 0\n\nanswer {\n generate: ai/generate-text\n}\n");
    let fixtures = generate_text_base_fixtures();
    let mut shared = fixtures[0].advertisement.clone();
    let mut exclusive = fixtures[1].advertisement.clone();
    exclusive.capabilities[0].implementation.implementation_id = shared.capabilities[0]
        .implementation
        .implementation_id
        .clone();
    assert_ne!(
        shared.capabilities[0].implementation.artifact_id,
        exclusive.capabilities[0].implementation.artifact_id
    );
    exclusive
        .resources
        .iter_mut()
        .find_map(|pool| pool.compute.as_mut())
        .expect("compute contract exists")
        .service_guarantee = ComputeServiceGuarantee::Exclusive;
    shared.capabilities[0].capability_id = conduit_core::CapabilityId::from("shared-compute");
    exclusive.capabilities[0].capability_id = conduit_core::CapabilityId::from("exclusive-compute");
    let choice = select_realization_with_policy(
        &checked.gears[0],
        &[shared, exclusive.clone()],
        &HardRealizationRequirements::default(),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::MaximizeComputeServiceGuarantee(
                ResourceClassId::from(CPU_EXECUTION_RESOURCE),
            )],
        },
    )
    .expect("equal-face artifacts remain selectable by compute policy");
    assert_eq!(choice.host_id, exclusive.host_id);
    assert_eq!(choice.capability_id.as_str(), "exclusive-compute");
}
