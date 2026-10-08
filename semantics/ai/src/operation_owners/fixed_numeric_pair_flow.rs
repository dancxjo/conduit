//! Hosted owners for explicitly closing-Flow typed pairing.
use crate::{fixed_numeric_pair_flow::*, fixed_numeric_preparation::verify_fixed_placement};
use alloc::collections::BTreeMap;
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;

pub struct FixedFlowPairOperationFactory {
    identity: ImplementationId,
    selected: BTreeMap<PlacementId, CapabilityOffer>,
}
impl FixedFlowPairOperationFactory {
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("Flow pairing requires one sealed fragment".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-pair"))
        {
            let offer = fixed_flow_pair_offer(gear.kind_id.as_str())?;
            verify_fixed_placement(gear, &offer).map_err(|error| format!("{error:?}"))?;
            selected.insert(gear.placement_id.clone(), offer);
        }
        Ok(Self {
            identity: ImplementationId::from(FLOW_PAIR_IMPLEMENTATION),
            selected,
        })
    }
    fn verify(&self, gear: &PlannedGear) -> Result<(), String> {
        let offer = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected Flow pair placement")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))
    }
}
impl KernelOperationFactory for FixedFlowPairOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.identity
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.verify(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: gear.limits.max_queue_bytes,
            maximum_value_bytes: gear.limits.max_queue_bytes,
            host_requests: 0,
            sign_items: 1,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        self.verify(gear)?;
        Ok(Box::new(
            FixedFlowPairBack::prepare_planned::<PORTS>(gear, 3)
                .map_err(|error| format!("{error:?}"))?,
        ))
    }
}
