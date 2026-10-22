use conduit_composite::{
    KernelCompositeSignStorage, KernelOperationBudget, KernelOperationFactory,
    KernelOperationRegistry, PreparedRegionKernel,
};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, CordId, HostedValueStore, KernelEventKind};
use conduit_plan_lowering::{
    execution_regions::*, lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};
#[path = "support/allocation.rs"]
mod allocation;
#[path = "support/region_fixture.rs"]
mod fixture;
use fixture::fixtures;
use fixture::{plan, Factory};
struct Unreported(Factory);
impl KernelOperationFactory for Unreported {
    fn implementation_id(&self) -> &ImplementationId {
        self.0.implementation_id()
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.0.budget(placement)
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        self.0.prepare(placement, values)
    }
}

#[test]
fn selected_pure_region_owners_use_existing_child_storage_and_transport_without_growth() {
    fn assert_send<T: Send>() {}
    assert_send::<PreparedRegionKernel>();
    let plan = plan();
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
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        })
        .unwrap();
    let mut source = PreparedRegionKernel::prepare(
        &prepared,
        0,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .unwrap();
    let mut sink = PreparedRegionKernel::prepare(
        &prepared,
        1,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .unwrap();
    assert_eq!(source.plan_id(), &plan.plan_id);
    assert_eq!(source.fragment_id(), &plan.fragments[0].fragment_id);
    let receipt = source.memory_receipt();
    assert_eq!(receipt.scratch_bytes, source.scratch_mut().len());
    assert_eq!(
        receipt.stack_reservation_bytes,
        source.worker_storage().stack_bytes as usize
    );
    assert!(receipt.total_admitted_bytes <= source.worker_storage().binding.units as usize);
    assert_eq!(
        receipt.total_admitted_bytes,
        receipt.kernel_backing_bytes
            + receipt.identity_backing_bytes
            + receipt.scratch_bytes
            + receipt.stack_reservation_bytes
    );
    let mut unreported = KernelOperationRegistry::new();
    unreported
        .install(Unreported(Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        }))
        .unwrap();
    let refusal = PreparedRegionKernel::prepare(
        &prepared,
        0,
        &unreported,
        KernelCompositeSignStorage::default(),
    )
    .err()
    .unwrap();
    assert!(
        refusal.contains("does not report worker Back memory"),
        "{refusal}"
    );
    let mut bytes = Vec::with_capacity(2);
    allocation::assert_no_allocations("region execution", || {
        source.step().unwrap();
        let sequence = source.offer_into(CordId(0), &mut bytes).unwrap().unwrap();
        assert_eq!(bytes, &[42]);
        sink.admit(CordId(0), sequence, &bytes).unwrap();
        source.delivered(CordId(0), sequence).unwrap();
        sink.step().unwrap();
        source.step().unwrap();
        assert!(source.terminal(CordId(0)).unwrap().is_some());
        sink.close(CordId(0)).unwrap();
        source.cancel().unwrap();
        sink.cancel().unwrap();
        assert_eq!(
            source.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Cancelled
        );
    });
    assert!(sink
        .signs()
        .iter()
        .any(|e| e.kind == KernelEventKind::ValueConsumed));
    let mut tight = plan.fragments[0].clone();
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut tight.execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.binding.units = u32::try_from(receipt.total_admitted_bytes - 1).unwrap();
    let tight = fixtures::common::seal(tight);
    let prepared_tight = PreparedExecutionRegions::prepare(
        &tight,
        &tight.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let refusal = PreparedRegionKernel::prepare(
        &prepared_tight,
        0,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .err()
    .unwrap();
    assert!(refusal.contains("complete managed backing"), "{refusal}");
    let mut unknown = fixtures::transfer_plan().fragments.remove(0);
    for region in &mut unknown.execution_regions {
        let ExecutionScheduling::PartitionedBoundedStep { storage } = &mut region.scheduling else {
            panic!()
        };
        storage.binding.units = 4 * 1024 * 1024;
    }
    let unknown = fixtures::common::seal(unknown);
    let prepared = PreparedExecutionRegions::prepare(
        &unknown,
        &unknown.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let refusal = PreparedRegionKernel::prepare(
        &prepared,
        0,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .err()
    .unwrap();
    assert!(refusal.contains("movement is ineligible"), "{refusal}");
    let mut tiny = plan.fragments[0].clone();
    let ExecutionScheduling::PartitionedBoundedStep { storage } =
        &mut tiny.execution_regions[0].scheduling
    else {
        panic!()
    };
    storage.binding.units = storage.stack_bytes + storage.scratch_bytes;
    let tiny = fixtures::common::seal(tiny);
    let prepared = PreparedExecutionRegions::prepare(
        &tiny,
        &tiny.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let refusal = PreparedRegionKernel::prepare(
        &prepared,
        0,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .err()
    .unwrap();
    assert!(refusal.contains("fixed kernel"), "{refusal}");
}
