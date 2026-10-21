use conduit_composite::{
    KernelCompositeSignStorage, KernelOperationBudget, KernelOperationFactory,
    KernelOperationRegistry, PreparedRegionKernel,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, CordId, HostedValueStore, KernelEventKind, PortId,
};
use conduit_plan_lowering::{
    execution_regions::*, lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};
#[path = "support/allocation.rs"]
mod allocation;
#[allow(dead_code)]
#[path = "../../plan-lowering/tests/common/execution_regions.rs"]
mod fixtures;
struct Back {
    source: bool,
    sent: bool,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.source {
            if self.sent {
                return StepOutcome::Complete;
            }
            io.send_canonical(PortId(0), CanonicalValue::new(&[42]).unwrap())
                .unwrap();
        } else if let Some(value) = bytes.input(PortId(0)) {
            assert_eq!(value, &[42]);
            io.consume(PortId(0)).unwrap();
        } else {
            return StepOutcome::Await;
        }
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.source {
            self.sent = true;
        }
    }
}
struct Factory {
    implementation: ImplementationId,
}
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 1,
            maximum_value_bytes: 1,
            host_requests: 0,
            sign_items: 8,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        Ok(Box::new(Back {
            source: placement.inputs.is_empty(),
            sent: false,
        }))
    }
}
fn plan() -> Plan {
    let mut fragment = fixtures::transfer_plan().fragments.remove(0);
    for placement in &mut fragment.placements {
        placement.semantic_contract.laws = vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ];
    }
    for region in &mut fragment.execution_regions {
        let ExecutionScheduling::PartitionedBoundedStep { storage } = &mut region.scheduling else {
            panic!()
        };
        storage.binding.units = 4 * 1024 * 1024;
    }
    fixtures::common::seal(fragment)
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
