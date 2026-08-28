use conduit_core::{ImplementationId, PlannedGear};
use conduit_kernel::{
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
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
    ) -> Result<Box<dyn Operation + Send>, String>;
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

pub(crate) struct BoxedKernelOperation(Box<dyn Operation + Send>);

impl BoxedKernelOperation {
    pub(crate) fn new(operation: Box<dyn Operation + Send>) -> Self {
        Self(operation)
    }

    pub(crate) fn inactive() -> Self {
        Self(Box::new(Inactive))
    }
}

impl Operation for BoxedKernelOperation {
    fn start(&mut self) -> OperationAction {
        self.0.start()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.0.resume(input)
    }

    fn accepts_input_while_host_operation_pending(&self) -> bool {
        self.0.accepts_input_while_host_operation_pending()
    }

    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        self.0.take_host_operation_cancellation()
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, bytes: &[u8]) -> OperationAction {
        self.0.resume_value(port, value, bytes)
    }

    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: conduit_kernel::HostOperationOutcome,
        bytes: Option<&[u8]>,
    ) -> OperationAction {
        self.0.resume_host_operation(request, outcome, bytes)
    }

    fn advance(&mut self) -> OperationAction {
        self.0.advance()
    }

    fn retains_resumed_value(&self) -> bool {
        self.0.retains_resumed_value()
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.0.take_released_value()
    }

    fn cancel(&mut self) {
        self.0.cancel();
    }
}

struct Inactive;

impl Operation for Inactive {
    fn start(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        OperationAction::Complete
    }
}
