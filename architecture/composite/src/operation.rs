use crate::prelude::*;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use conduit_core::{ImplementationId, PlannedGear};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::HostedValueStore;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelOperationBudget {
    pub value_items: u16,
    pub value_bytes: u32,
    pub maximum_value_bytes: u32,
    pub host_requests: u16,
    pub sign_items: u16,
}

/// Explicit reusable pure-Value operation; mutable instances are never cloned.
pub type ReusableKernelOperation = Box<
    dyn conduit_kernel::scheduler::prepared_session::ReusableStepBack<
            { FIXED_KERNEL_STORAGE_PORTS_PER_NODE },
            Next = (),
        > + Send,
>;

pub trait KernelOperationFactory: Send + Sync {
    fn implementation_id(&self) -> &ImplementationId;
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String>;
    fn prepare(
        &self,
        placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String>;
    /// Opt-in only after exact preparation/profile admission. Unsupported or
    /// failed reuse is a refusal, never an implicit one-shot retry during Play.
    fn prepare_reusable_value(
        &self,
        _placement: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<ReusableKernelOperation, String> {
        Err("installed operation does not admit reusable Values".into())
    }
}

#[derive(Default, Clone)]
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

pub(crate) enum BoxedKernelBack {
    Single(Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>),
    Reusable(ReusableKernelOperation),
}
impl BoxedKernelBack {
    pub(crate) fn new(
        back: Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>,
    ) -> Self {
        Self::Single(back)
    }
    pub(crate) fn reusable(back: ReusableKernelOperation) -> Self {
        Self::Reusable(back)
    }
    pub(crate) fn inactive() -> Self {
        Self::Single(Box::new(Inactive))
    }
    fn back(&self) -> &(dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send) {
        match self {
            Self::Single(back) => &**back,
            Self::Reusable(back) => &**back,
        }
    }
    fn back_mut(&mut self) -> &mut (dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send) {
        match self {
            Self::Single(back) => &mut **back,
            Self::Reusable(back) => &mut **back,
        }
    }
}
impl
    conduit_kernel::scheduler::prepared_session::ReusableStepBack<
        { FIXED_KERNEL_STORAGE_PORTS_PER_NODE },
    > for BoxedKernelBack
{
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn conduit_kernel::ValueStorage) -> bool {
        match self {
            Self::Reusable(back) => back.preflight_next(next, values),
            Self::Single(_) => false,
        }
    }
    fn begin_next(&mut self, next: ()) {
        if let Self::Reusable(back) = self {
            back.begin_next(next);
        }
    }
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for BoxedKernelBack {
    fn terminal_transductions(
        &self,
    ) -> [Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;
           FIXED_KERNEL_STORAGE_PORTS_PER_NODE] {
        self.back().terminal_transductions()
    }
    fn terminal_transduction(
        &self,
    ) -> Option<conduit_kernel::scheduler::AssignedTerminalTransduction> {
        self.back().terminal_transduction()
    }
    fn prepared_output(&self, port: conduit_kernel::PortId) -> Option<&[u8]> {
        self.back().prepared_output(port)
    }
    fn step_committed(&mut self) {
        self.back_mut().step_committed();
    }
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        self.back_mut().step(io, input_bytes)
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.back().accepts_input_while_host_call_pending()
    }
    fn retains_host_call_input(
        &self,
        request: conduit_kernel::RequestId,
        value: conduit_kernel::ValueRef,
    ) -> bool {
        self.back().retains_host_call_input(request, value)
    }

    fn cancel(&mut self) {
        self.back_mut().cancel();
    }
}

#[cfg(test)]
mod tests;

struct Inactive;

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Inactive {
    fn step(
        &mut self,
        _io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        StepOutcome::Complete
    }
}
