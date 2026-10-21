use conduit_core::*;
use conduit_plan_lowering::execution_regions::*;
#[path = "common/execution_regions.rs"]
mod fixtures;

fn limits() -> ExecutionRegionLimits {
    ExecutionRegionLimits {
        maximum_regions: 2,
        maximum_lanes: 2,
        maximum_nodes: 2,
        maximum_cords: 1,
    }
}
#[test]
fn worker_memory_is_sealed_and_cumulative_demand_requires_current_capacity() {
    let plan = fixtures::transfer_plan();
    let prepared =
        PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits()).unwrap();
    let (mut host, mut observations) = fixtures::lane_host(&plan, 2);
    assert_eq!(
        prepared
            .check_resource_offers(&host, &observations)
            .unwrap()
            .bindings()
            .len(),
        4
    );
    let memory = host
        .resources
        .iter_mut()
        .find(|o| o.pool_id.as_str() == "worker-memory")
        .unwrap();
    memory.capacity_units /= 2;
    let memory = observations
        .iter_mut()
        .find(|o| o.pool_id.as_str() == "worker-memory")
        .unwrap();
    memory.unreserved_units /= 2;
    assert!(matches!(
        prepared.check_resource_offers(&host, &observations),
        Err(RegionResourceOfferError::CapacityExceeded)
    ));
    let (host, mut observations) = fixtures::lane_host(&plan, 2);
    observations
        .iter_mut()
        .find(|o| o.pool_id.as_str() == "worker-memory")
        .unwrap()
        .offer_generation = OfferGeneration(0);
    assert!(matches!(
        prepared.check_resource_offers(&host, &observations),
        Err(RegionResourceOfferError::StaleObservation)
    ));
    let mut changed = plan.clone();
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut changed.fragments[0].execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.stack_bytes += 1;
    assert!(!verify_plan(&changed));
    let changed = fixtures::common::seal(changed.fragments.remove(0));
    assert!(verify_plan(&changed));
    assert_ne!(changed.plan_id, plan.plan_id);
    let mut changed = plan.clone();
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut changed.fragments[0].execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.binding.pool_id = "other-memory".into();
    assert!(!verify_plan(&changed));
    let changed = fixtures::common::seal(changed.fragments.remove(0));
    assert_ne!(changed.plan_id, plan.plan_id);
}
#[test]
fn worker_stack_scratch_and_value_backing_must_fit_the_separate_binding() {
    let mut fragment = fixtures::transfer_plan().fragments.remove(0);
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut fragment.execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.stack_bytes = 0;
    assert!(!verify_plan(&fixtures::common::seal(fragment)));
    let mut fragment = fixtures::transfer_plan().fragments.remove(0);
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut fragment.execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.stack_bytes = u32::MAX;
    storage.scratch_bytes = 1;
    assert!(!verify_plan(&fixtures::common::seal(fragment)));
    #[cfg(feature = "hosted-region-storage")]
    {
        let mut fragment = fixtures::transfer_plan().fragments.remove(0);
        let ExecutionScheduling::PartitionedBoundedStep { storage } =
            &mut fragment.execution_regions[0].scheduling
        else {
            panic!()
        };
        storage.binding.units = storage.stack_bytes + storage.scratch_bytes;
        let plan = fixtures::common::seal(fragment);
        assert!(verify_plan(&plan));
        let prepared =
            PreparedExecutionRegions::prepare(&plan, &plan.fragments[0].fragment_id, limits())
                .unwrap();
        assert!(matches!(
            prepared.prepare_value_storage(RegionValueStorageLimits {
                maximum_retained_bytes: 4096,
                maximum_preparation_bytes: 4096,
            }),
            Err(RegionValueStorageError::CapacityExceeded)
        ));
    }
}
