//! Allocation-independent installation of the lowered ordinary text Plan.

use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, NodeId, Operation,
    OperationAction, OperationInput, PortId, RequestId, SignSink, ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_runtime::lowering::{LoweredPlanFragment, MAXIMUM_KERNEL_PORTS_PER_NODE};

const PRESENT_REQUEST: RequestId = RequestId(2);
const MAX_NODES: usize = 2;
const MAX_CORDS: usize = 1;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 1;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 1;
const HOST_BINDING_SLOTS: usize = 4;
const PENDING_REQUESTS: usize = 2;
const VALUE_SLOTS: usize = 4;
const VALUE_BYTES: usize = (conduit_std_catalog::MAX_TEXT_BYTES as usize) * 2;
const SIGN_CAPACITY: usize = 64;

type Driver = OperationDriver<PlannedOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiteralState {
    Emitting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiteralOperation {
    text: ValueRef,
    state: LiteralState,
}

impl Operation for LiteralOperation {
    fn start(&mut self) -> OperationAction {
        self.state = LiteralState::Emitting;
        OperationAction::Emit {
            port: PortId(0),
            value: self.text,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(11)
    }

    fn advance(&mut self) -> OperationAction {
        if self.state == LiteralState::Emitting {
            self.state = LiteralState::Complete;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.state = LiteralState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PresentationOperation {
    pending: bool,
    complete: bool,
}

impl Operation for PresentationOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                let Ok(input) = BoundedValueRef::new(value, conduit_std_catalog::MAX_TEXT_BYTES)
                else {
                    return invalid(20);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: PRESENT_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.complete = true;
                OperationAction::Await
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                OperationAction::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::Cancelled,
                    detail: 22,
                })
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Failed =>
            {
                OperationAction::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostOperationFailed,
                    detail: 23,
                })
            }
            OperationInput::Closed { port: PortId(0) } if self.complete && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(21),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedOperation {
    Literal(LiteralOperation),
    Presentation(PresentationOperation),
}

impl Operation for PlannedOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.start(),
            Self::Presentation(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.resume(input),
            Self::Presentation(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.advance(),
            Self::Presentation(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Literal(operation) => operation.cancel(),
            Self::Presentation(operation) => operation.cancel(),
        }
    }
}

pub struct TextPlannedKernel {
    scheduler: Scheduler,
    presentation_node: NodeId,
}

impl TextPlannedKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        validate_shape(fragment, lowered)?;
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let literal_index = fragment
            .placements
            .iter()
            .position(|placement| {
                placement.kind_id.as_str() == conduit_std_catalog::TEXT_LITERAL_KIND
            })
            .ok_or(SchedulerError::InvalidPlan)?;
        let presentation_index = fragment
            .placements
            .iter()
            .position(|placement| {
                placement.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND
            })
            .ok_or(SchedulerError::InvalidPlan)?;
        let literal = configured_text(&fragment.placements[literal_index].configuration, "value")?;
        let text = values.store(literal.as_bytes())?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = [lowered.cords[0].spec];
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes.install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(MAX_NODES as u16);
        for operation in &lowered.host_operations {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;
        let literal_driver = OperationDriver::new(PlannedOperation::Literal(LiteralOperation {
            text,
            state: LiteralState::Emitting,
        }))?;
        let presentation_driver =
            OperationDriver::new(PlannedOperation::Presentation(PresentationOperation {
                pending: false,
                complete: false,
            }))?;
        let drivers = if literal_index == 0 {
            [literal_driver, presentation_driver]
        } else {
            [presentation_driver, literal_driver]
        };
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            presentation_node: NodeId(presentation_index as u16),
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(value)
    }

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        if request.node != self.presentation_node
            || request.operation != conduit_kernel::HostOperationId(0)
        {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }
    #[cfg(test)]
    fn fail_presentation(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        if !self.is_presentation_request(&request) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostOperationFailed,
                    detail: 1,
                }),
            },
        )
    }
    pub fn is_presentation_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.presentation_node
            && request.operation == conduit_kernel::HostOperationId(0)
    }

    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.scheduler.cancel()
    }
    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }
    pub fn sign_count(&self) -> u16 {
        self.scheduler.signs().len()
    }
    pub fn pending_host_operations(&self) -> usize {
        self.scheduler.pending_host_operation_count()
    }
}

fn configured_u64(
    entries: &[conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<u64, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or(SchedulerError::InvalidPlan)
}

fn configured_text<'a>(
    entries: &'a [conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<&'a str, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::Text(value)) if candidate == key => {
                Some(value.as_str())
            }
            _ => None,
        })
        .filter(|value| {
            value.len() <= conduit_std_catalog::MAX_TEXT_BYTES as usize
                && core::str::from_utf8(value.as_bytes()).is_ok()
        })
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<(), SchedulerError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.routes.len() != 1
        || lowered.host_operations.len() != 1
        || lowered.cord_value_slots != 1
        || lowered.cord_value_bytes != crate::ordinary_plan::TEXT_LITERAL.len() as u32
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(SchedulerError::InvalidPlan);
    }
    let literal = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::TEXT_LITERAL_KIND)
        .ok_or(SchedulerError::InvalidPlan)?;
    let presentation = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND)
        .ok_or(SchedulerError::InvalidPlan)?;
    if literal.implementation_id.as_str() != crate::offer::TEXT_LITERAL_IMPLEMENTATION
        || presentation.implementation_id.as_str() != crate::offer::TEXT_PRESENTATION_IMPLEMENTATION
        || configured_text(&literal.configuration, "value")? != crate::ordinary_plan::TEXT_LITERAL
        || configured_u64(&presentation.configuration, "maximum-values")?
            != conduit_std_catalog::MAX_TEXT_VALUES
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        offer::{CpuFeatures, HostOffer},
        ordinary_plan,
    };

    fn kernel() -> TextPlannedKernel {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        ordinary_plan::prepare(&identities, &offer, "build")
            .unwrap()
            .kernel
    }

    #[test]
    fn ordinary_cancellation_is_terminal() {
        let mut kernel = kernel();
        kernel.cancel().unwrap();
        assert_eq!(kernel.step(), Ok(SchedulerStatus::Cancelled));
    }

    #[test]
    fn malformed_presentation_completion_is_rejected() {
        let mut kernel = kernel();
        assert_eq!(
            kernel.complete_presentation(HostOperationRequest {
                node: NodeId(99),
                request: RequestId(99),
                operation: conduit_kernel::HostOperationId(0),
                input: BoundedValueRef::new(
                    ValueRef {
                        slot: 0,
                        generation: 0,
                        byte_len: 1,
                    },
                    1,
                )
                .unwrap(),
            }),
            Err(SchedulerError::InvalidHostOperationAccess)
        );
    }

    #[test]
    fn presentation_base_loss_remains_a_distinct_terminal_failure() {
        let mut kernel = kernel();
        let request = loop {
            assert!(matches!(
                kernel.step(),
                Ok(SchedulerStatus::Progress { .. })
            ));
            if let Some(request) = kernel.next_host_request() {
                break request;
            }
        };
        kernel.fail_presentation(request).unwrap();
        assert_eq!(kernel.step(), Err(SchedulerError::OperationFailed(23)));
    }
}
