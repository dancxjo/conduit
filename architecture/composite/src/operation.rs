use conduit_core::{ImplementationId, PlannedGear};
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::HostedValueStore;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelOperationBudget {
    pub value_items: u16,
    pub value_bytes: u32,
    pub maximum_value_bytes: u32,
    pub host_requests: u16,
    pub sign_items: u16,
}

pub trait KernelOperationFactory: Send + Sync {
    fn implementation_id(&self) -> &ImplementationId;
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String>;
    fn prepare(
        &self,
        placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepOperation<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String>;
}

#[derive(Default)]
pub struct KernelOperationRegistry {
    factories: BTreeMap<ImplementationId, Arc<dyn KernelOperationFactory>>,
}

impl KernelOperationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install(
        &mut self,
        factory: impl KernelOperationFactory + 'static,
    ) -> Result<(), String> {
        let implementation_id = factory.implementation_id().clone();
        if implementation_id.as_str().is_empty() || self.factories.contains_key(&implementation_id)
        {
            return Err(format!(
                "duplicate or empty kernel implementation '{}'",
                implementation_id.as_str()
            ));
        }
        self.factories.insert(implementation_id, Arc::new(factory));
        Ok(())
    }

    pub(crate) fn get(
        &self,
        implementation_id: &ImplementationId,
    ) -> Option<&Arc<dyn KernelOperationFactory>> {
        self.factories.get(implementation_id)
    }
}

pub(crate) struct BoxedKernelBack(
    Box<dyn StepOperation<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>,
);

impl BoxedKernelBack {
    pub(crate) fn new(
        back: Box<dyn StepOperation<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>,
    ) -> Self {
        Self(back)
    }

    pub(crate) fn inactive() -> Self {
        Self(Box::new(Inactive))
    }
}

impl StepOperation<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for BoxedKernelBack {
    fn step_committed(&mut self) {
        self.0.step_committed();
    }
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        self.0.step(io, input_bytes)
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.0.accepts_input_while_host_call_pending()
    }
    fn retains_host_call_input(
        &self,
        request: conduit_kernel::RequestId,
        value: conduit_kernel::ValueRef,
    ) -> bool {
        self.0.retains_host_call_input(request, value)
    }

    fn cancel(&mut self) {
        self.0.cancel();
    }
}

struct Inactive;

impl StepOperation<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Inactive {
    fn step(
        &mut self,
        _io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        StepOutcome::Complete
    }
}
