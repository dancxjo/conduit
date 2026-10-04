//! Shared selector Back with exact prepared value ceilings.
use super::*;
use alloc::{boxed::Box, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct SelectorOperationFactory {
    implementation: ImplementationId,
}
impl Default for SelectorOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
        }
    }
}
impl KernelOperationFactory for SelectorOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        selected_selector(gear).map_err(|error| format!("native selector refusal: {error:?}"))?;
        let call = &gear.host_calls[0];
        let maximum = call
            .maximum_input_bytes
            .max(call.maximum_output_bytes)
            .max(1);
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: maximum * 4,
            maximum_value_bytes: maximum,
            host_requests: 1,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.budget(gear)?;
        Ok(Box::new(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                gear.host_calls[0].maximum_input_bytes,
            ),
        ))
    }
}
