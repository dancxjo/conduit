use conduit_core::*;
use conduit_plan_lowering::execution_regions::*;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn plan(lanes: u32) -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    fragment.execution_regions.push(ExecutionRegion {
        region_id: ExecutionRegionId::from("region"),
        admitted_placements: vec![PlacementId::from("placement")],
        execution_profile_id: ExecutionProfileId::from("fixture/cooperative@1"),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: lanes,
        lane_resource: ResourceBinding {
            pool_id: ResourcePoolId::from("lanes"),
            class_id: ResourceClassId::from("conduit.resource/execution-lane@1"),
            units: lanes,
            protected: None,
            content: None,
            compute: Some(ComputeReservation {
                selected_lanes: lanes,
                service_guarantee: ComputeServiceGuarantee::Shared,
                architecture_base_id: ArchitectureBaseId::from("base"),
                architecture_base_kind: ArchitectureBaseKind::HostedOs,
                topology_group_id: None,
                performance_class: None,
                nominal_clock_hz: None,
            }),
        },
        lane_base_id: HostBaseId::from("base"),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: 0,
            timer_slots: 0,
            cord_item_capacity: 0,
            cord_byte_capacity: 0,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        },
        preemption_required: false,
        isolation_required: false,
    });
    common::seal(fragment)
}

#[test]
fn sealed_partition_preserves_identity_and_obeys_backend_capacity() {
    let limits = ExecutionRegionLimits {
        maximum_regions: 1,
        maximum_lanes: 4,
        maximum_nodes: 1,
        maximum_cords: 0,
    };
    for lanes in [1, 2, 4] {
        let plan = plan(lanes);
        assert!(verify_plan(&plan));
        let id = &plan.fragments[0].fragment_id;
        let prepared = PreparedExecutionRegions::prepare(&plan, id, limits).unwrap();
        assert!(std::ptr::eq(prepared.plan(), &plan));
        assert!(std::ptr::eq(prepared.fragment(), &plan.fragments[0]));
        assert!(std::ptr::eq(
            prepared.regions()[0].admitted(),
            &plan.fragments[0].execution_regions[0]
        ));
        assert_eq!(prepared.requested_lanes(), lanes);
        assert_eq!(
            prepared.regions()[0].nodes(),
            &[prepared.lowered().nodes[0].node]
        );
        assert_eq!(prepared.lowered().identity.plan_id, plan.plan_id);
        assert!(prepared.activations().entries.is_empty());
        assert!(matches!(
            PreparedExecutionRegions::prepare(
                &plan,
                id,
                ExecutionRegionLimits {
                    maximum_lanes: lanes - 1,
                    ..limits
                }
            ),
            Err(RegionPreparationError::CapacityExceeded)
        ));
        let mut stale = plan.clone();
        stale.fragments[0].execution_regions[0].lane_count += 1;
        assert!(matches!(
            PreparedExecutionRegions::prepare(
                &stale,
                id,
                ExecutionRegionLimits {
                    maximum_lanes: 8,
                    ..limits
                }
            ),
            Err(RegionPreparationError::Lowering(_))
        ));
    }
}

#[test]
fn independent_regions_keep_original_nodes_and_refuse_cross_region_startup() {
    let mut fragment = plan(1).fragments.remove(0);
    let mut placement = fragment.placements[0].clone();
    placement.placement_id = PlacementId::from("second");
    placement.gear_id = GearId::from("second");
    fragment.startup_order.push(placement.placement_id.clone());
    let mut region = fragment.execution_regions[0].clone();
    region.region_id = ExecutionRegionId::from("second");
    region.admitted_placements = vec![placement.placement_id.clone()];
    fragment.placements.push(placement);
    fragment.execution_regions.push(region);
    let limits = ExecutionRegionLimits {
        maximum_regions: 2,
        maximum_lanes: 2,
        maximum_nodes: 2,
        maximum_cords: 0,
    };
    let plan = common::seal(fragment.clone());
    let prepared =
        PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits).unwrap();
    assert_eq!(prepared.regions().len(), 2);
    assert_ne!(prepared.regions()[0].nodes(), prepared.regions()[1].nodes());
    assert_eq!(prepared.requested_lanes(), 2);
    fragment.startup_dependencies.push(StartupDependency {
        prerequisite_placement_id: PlacementId::from("placement"),
        dependent_placement_id: PlacementId::from("second"),
    });
    let plan = common::seal(fragment);
    assert!(verify_plan(&plan));
    assert!(matches!(
        PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits),
        Err(RegionPreparationError::UnsupportedCrossRegionDependency)
    ));
}

fn lane_host(plan: &Plan, capacity: u32) -> (HostAdvertisement, Vec<ResourceObservation>) {
    let fragment = &plan.fragments[0];
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("fixture"),
        bases: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
        resources: vec![ResourceOffer {
            pool_id: ResourcePoolId::from("lanes"),
            class_id: ResourceClassId::from("conduit.resource/execution-lane@1"),
            capacity_units: capacity,
            content: None,
            compute: Some(ComputePoolContract {
                service_guarantee: ComputeServiceGuarantee::Shared,
                architecture_base_id: ArchitectureBaseId::from("base"),
                architecture_base_kind: ArchitectureBaseKind::HostedOs,
                topology_groups: vec![],
            }),
        }],
    };
    let observations = vec![ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: host.resources[0].pool_id.clone(),
        class_id: host.resources[0].class_id.clone(),
        health: ResourceHealth::Ready,
        unreserved_units: capacity,
        utilized_units: 0,
        sign_id: SignId::from("current"),
    }];
    (host, observations)
}

#[test]
fn lane_offers_check_current_capacity_and_aggregate_shared_pool_demand() {
    let limits = ExecutionRegionLimits {
        maximum_regions: 2,
        maximum_lanes: 4,
        maximum_nodes: 2,
        maximum_cords: 0,
    };
    for lanes in [1, 2, 4] {
        let plan = plan(lanes);
        let prepared =
            PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits)
                .unwrap();
        for capacity in [1, 2, 4] {
            let (host, mut observations) = lane_host(&plan, capacity);
            assert_eq!(
                prepared.check_lane_offers(&host, &observations).is_ok(),
                lanes <= capacity
            );
            if lanes > capacity {
                continue;
            }
            let checked = prepared.check_lane_offers(&host, &observations).unwrap();
            assert!(std::ptr::eq(
                checked.bindings()[0],
                &plan.fragments[0].execution_regions[0].lane_resource
            ));
            observations[0].unreserved_units = lanes - 1;
            assert!(matches!(
                prepared.check_lane_offers(&host, &observations),
                Err(LaneOfferError::CapacityExceeded)
            ));
            observations[0].offer_generation = OfferGeneration(0);
            assert!(matches!(
                prepared.check_lane_offers(&host, &observations),
                Err(LaneOfferError::StaleObservation)
            ));
            let (mut stale, observations) = lane_host(&plan, capacity);
            stale.boot_id = BootId::from("other");
            assert!(matches!(
                prepared.check_lane_offers(&stale, &observations),
                Err(LaneOfferError::StaleHost)
            ));
        }
    }
    let mut fragment = plan(1).fragments.remove(0);
    let mut second = fragment.placements[0].clone();
    second.placement_id = PlacementId::from("second");
    second.gear_id = GearId::from("second");
    fragment.startup_order.push(second.placement_id.clone());
    let mut region = fragment.execution_regions[0].clone();
    region.region_id = ExecutionRegionId::from("second");
    region.admitted_placements = vec![second.placement_id.clone()];
    fragment.placements.push(second);
    fragment.execution_regions.push(region);
    let plan = common::seal(fragment);
    let prepared =
        PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits).unwrap();
    let (host, observations) = lane_host(&plan, 1);
    assert!(matches!(
        prepared.check_lane_offers(&host, &observations),
        Err(LaneOfferError::CapacityExceeded)
    ));
    let (mut host, observations) = lane_host(&plan, 2);
    assert!(prepared.check_lane_offers(&host, &observations).is_ok());
    // A pool can have spare total lanes while one selected topology group
    // is full. The same aggregate accounting must hold inside that group.
    let mut fragment = plan.fragments[0].clone();
    for region in &mut fragment.execution_regions {
        region
            .lane_resource
            .compute
            .as_mut()
            .unwrap()
            .topology_group_id = Some(ComputeTopologyGroupId::from("group"));
    }
    let grouped = common::seal(fragment);
    let prepared_group =
        PreparedExecutionRegions::prepare(&grouped, &grouped.fragments[0].fragment_id, limits)
            .unwrap();
    let (mut grouped_host, grouped_observations) = lane_host(&grouped, 4);
    grouped_host.resources[0]
        .compute
        .as_mut()
        .unwrap()
        .topology_groups
        .push(ComputeTopologyGroup {
            group_id: ComputeTopologyGroupId::from("group"),
            lane_capacity: 1,
            numa_domain: None,
            cache_domain: None,
            performance_class: None,
            nominal_clock_hz: None,
        });
    assert!(matches!(
        prepared_group.check_lane_offers(&grouped_host, &grouped_observations),
        Err(LaneOfferError::CapacityExceeded)
    ));
    grouped_host.resources[0]
        .compute
        .as_mut()
        .unwrap()
        .topology_groups[0]
        .lane_capacity = 2;
    assert!(prepared_group
        .check_lane_offers(&grouped_host, &grouped_observations)
        .is_ok());
    host.resources.push(host.resources[0].clone());
    assert!(matches!(
        prepared.check_lane_offers(&host, &observations),
        Err(LaneOfferError::AmbiguousPool)
    ));
}
