//! The native expression realization uses the shared kernel's Host Call lifecycle.
use super::{IMPLEMENTATION, prepared_program};
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{ImplementationId, PlannedGear};
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct ExpressionOperationFactory {
    implementation: ImplementationId,
}
impl Default for ExpressionOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
        }
    }
}
impl KernelOperationFactory for ExpressionOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let program = prepared_program(placement).map_err(|error| format!("{error:?}"))?;
        let input = program
            .maximum_prepared_input_bytes()
            .map_err(|error| format!("{error:?}"))?;
        let output = program
            .maximum_prepared_output_bytes()
            .map_err(|error| format!("{error:?}"))?;
        let maximum = input.max(output).max(1);
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
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.budget(placement)?;
        Ok(Box::new(HostCallBack::new(
            placement.host_calls[0].maximum_input_bytes,
        )))
    }
}
