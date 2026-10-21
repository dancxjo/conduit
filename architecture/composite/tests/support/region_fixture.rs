use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, HostedValueStore, PortId,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
#[allow(dead_code)]
#[path = "../../../plan-lowering/tests/common/execution_regions.rs"]
pub(crate) mod fixtures;
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
pub(crate) struct Factory {
    pub(crate) implementation: ImplementationId,
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
    fn prepare_worker(
        &self,
        placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<conduit_composite::PreparedWorkerBack, String> {
        conduit_composite::PreparedWorkerBack::new(self.prepare(placement, values)?, 0)
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

pub(crate) fn plan() -> Plan {
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
