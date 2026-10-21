use conduit_core::*;
use conduit_plan_lowering::execution_regions::*;
#[path = "common/execution_regions.rs"]
mod fixtures;
use fixtures::{common, plan, transfer_plan};

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

#[test]
fn typed_transfer_requires_explicit_profile_and_exact_destination_storage() {
    use conduit_plan_lowering::lowering::{lower_plan_fragment_from_plan, LoweringError};
    let plan = transfer_plan();
    assert!(verify_plan(&plan));
    let id = &plan.fragments[0].fragment_id;
    assert!(matches!(
        lower_plan_fragment_from_plan(&plan, id),
        Err(LoweringError::UnsupportedExecutionRegion(_))
    ));
    let limits = ExecutionRegionLimits {
        maximum_regions: 2,
        maximum_lanes: 2,
        maximum_nodes: 2,
        maximum_cords: 1,
    };
    let prepared = PreparedExecutionRegions::prepare(&plan, id, limits).unwrap();
    let transfer = &prepared.transfers()[0];
    assert!(std::ptr::eq(
        transfer.connection(),
        &plan.fragments[0].connections[0]
    ));
    assert_eq!((transfer.source_region(), transfer.sink_region()), (0, 1));
    assert_eq!(transfer.cord(), prepared.lowered().cords[0].spec.cord);
    assert_eq!(prepared.regions()[0].cords(), &[transfer.cord()]);
    assert_eq!(prepared.regions()[1].cords(), &[transfer.cord()]);
    assert!(matches!(
        PreparedExecutionRegions::prepare(
            &plan,
            id,
            ExecutionRegionLimits {
                maximum_cords: 0,
                ..limits
            }
        ),
        Err(RegionPreparationError::CapacityExceeded)
    ));
    let mut uncharged_source = plan.fragments[0].clone();
    uncharged_source.execution_regions[0]
        .requirements
        .cord_item_capacity = 0;
    uncharged_source.execution_regions[0]
        .requirements
        .cord_byte_capacity = 0;
    assert!(!verify_plan(&common::seal(uncharged_source)));
    let mut fragment = plan.fragments[0].clone();
    fragment.execution_regions[1]
        .requirements
        .cord_byte_capacity = 1;
    assert!(!verify_plan(&common::seal(fragment)));
    let mut fragment = plan.fragments[0].clone();
    fragment.execution_regions[0].scheduling = ExecutionScheduling::CooperativeBoundedStep;
    assert!(!verify_plan(&common::seal(fragment)));
    let mut fragment = plan.fragments[0].clone();
    for region in &mut fragment.execution_regions {
        region.scheduling = ExecutionScheduling::CooperativeBoundedStep;
        region.requirements.cord_item_capacity = 0;
        region.requirements.cord_byte_capacity = 0;
    }
    assert!(!verify_plan(&common::seal(fragment)));
    let mut reference = plan.fragments[0].clone();
    reference.execution_regions.truncate(1);
    reference.execution_regions[0].scheduling = ExecutionScheduling::CooperativeBoundedStep;
    reference.execution_regions[0]
        .admitted_placements
        .push(PlacementId::from("sink"));
    reference.execution_regions[0]
        .requirements
        .cord_item_capacity = 2;
    reference.execution_regions[0]
        .requirements
        .cord_byte_capacity = 2;
    let reference = common::seal(reference);
    assert!(verify_plan(&reference));
    assert!(lower_plan_fragment_from_plan(&reference, &reference.fragments[0].fragment_id).is_ok());
    let prepared_reference =
        PreparedExecutionRegions::prepare(&reference, &reference.fragments[0].fragment_id, limits)
            .unwrap();
    assert!(prepared_reference.transfers().is_empty());
    assert_eq!(prepared_reference.regions()[0].cords().len(), 1);
}

#[cfg(feature = "hosted-region-storage")]
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[cfg(feature = "hosted-region-storage")]
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[cfg(feature = "hosted-region-storage")]
#[test]
fn destination_value_storage_has_exact_quotas_and_no_use_time_growth() {
    use conduit_kernel::{StorageError, ValueStorage};
    let plan = transfer_plan();
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let limits = RegionValueStorageLimits {
        maximum_retained_bytes: 4096,
        maximum_preparation_bytes: 4096,
    };
    let mut storage = prepared.prepare_value_storage(limits).unwrap();
    assert!(std::ptr::eq(storage.prepared(), &prepared));
    assert!(storage.region_mut(0).unwrap().is_some());
    assert!(matches!(
        storage.region_mut(2),
        Err(RegionValueStorageError::UnknownRegion)
    ));
    let retained = storage.retained_bytes();
    let preparation = storage.preparation_bytes_upper_bound();
    let (_, observed) = allocation_probe::observe(|| {
        assert!(matches!(
            prepared.prepare_value_storage(RegionValueStorageLimits {
                maximum_retained_bytes: retained - 1,
                ..limits
            }),
            Err(RegionValueStorageError::CapacityExceeded)
        ));
        assert!(matches!(
            prepared.prepare_value_storage(RegionValueStorageLimits {
                maximum_preparation_bytes: preparation - 1,
                ..limits
            }),
            Err(RegionValueStorageError::CapacityExceeded)
        ));
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    let store = storage.region_mut(1).unwrap().unwrap();
    assert_eq!((store.item_capacity(), store.byte_capacity()), (2, 2));
    let capacity = store.allocation_capacities();
    let (_, observed) = allocation_probe::observe(|| {
        for _ in 0..100 {
            let first = store.store(&[1]).unwrap();
            let second = store.store(&[2]).unwrap();
            assert!(store.store(&[3]).is_err());
            assert_eq!(store.get(first).unwrap(), &[1]);
            assert_eq!(store.get(second).unwrap(), &[2]);
            store.release(first).unwrap();
            store.release(second).unwrap();
        }
        let full = store.store(&[1, 2]).unwrap();
        assert_eq!(store.store(&[3]), Err(StorageError::ByteCapacityExceeded));
        store.release(full).unwrap();
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    assert_eq!(store.allocation_capacities(), capacity);
}
