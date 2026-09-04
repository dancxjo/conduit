//! Fenced P2/P3 hand-lowered regression fixture; production uses `planned_kernel`.

use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, KernelEvent, NodeId, Operation, OperationAction, OperationInput, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueRef,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver,
        SchedulerError, SchedulerStatus,
    },
};

use crate::machine::KernelInterest;

pub const WAIT_OPERATION: HostOperationId = HostOperationId(0);
pub const PRESENT_OPERATION: HostOperationId = HostOperationId(0);
pub const TIMER_REQUEST: RequestId = RequestId(1);
pub const PRESENT_REQUEST: RequestId = RequestId(2);
pub const TIMER_VALUE: &[u8] = &0_u64.to_le_bytes();
pub const TIMER_WAIT: &[u8] = &1_u64.to_le_bytes();
pub const NODE_COUNT: usize = 2;
pub const CORD_COUNT: usize = 1;
pub const SIGN_CAPACITY: usize = 64;

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 1;
const ROUTE_SLOTS: usize = 1;
const ROUTE_TARGETS: usize = 1;
const HOST_BINDING_SLOTS: usize = 4;
const PENDING_REQUESTS: usize = 2;
const VALUE_SLOTS: usize = 4;
const VALUE_BYTES: usize = 64;

type Driver = OperationDriver<ProfileOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    NODE_COUNT,
    CORD_COUNT,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimerOperationState {
    Waiting,
    Emitting(ValueRef),
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerOperation {
    wait: BoundedValueRef,
    tick: ValueRef,
    state: TimerOperationState,
}

impl TimerOperation {
    fn new(wait: ValueRef, tick: ValueRef) -> Result<Self, SchedulerError> {
        Ok(Self {
            wait: BoundedValueRef::new(wait, 8)?,
            tick,
            state: TimerOperationState::Waiting,
        })
    }
}

impl Operation for TimerOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::RequestHostOperation {
            request: TIMER_REQUEST,
            operation: WAIT_OPERATION,
            input: self.wait,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TIMER_REQUEST
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none() =>
            {
                self.state = TimerOperationState::Emitting(self.tick);
                OperationAction::Emit {
                    port: PortId(0),
                    value: self.tick,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TIMER_REQUEST
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                OperationAction::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::Cancelled,
                    detail: 10,
                })
            }
            OperationInput::HostOperationCompleted { request, .. } if request == TIMER_REQUEST => {
                OperationAction::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostOperationFailed,
                    detail: 11,
                })
            }
            _ => OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 12,
            }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self.state {
            TimerOperationState::Emitting(_) => {
                self.state = TimerOperationState::Complete;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        self.state = TimerOperationState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SerialOperationState {
    Waiting,
    Presenting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SerialOperation {
    state: SerialOperationState,
}

impl Operation for SerialOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.state == SerialOperationState::Waiting => {
                let Ok(input) = BoundedValueRef::new(value, 16) else {
                    return OperationAction::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::InvalidInput,
                        detail: 20,
                    });
                };
                self.state = SerialOperationState::Presenting;
                OperationAction::RequestHostOperation {
                    request: PRESENT_REQUEST,
                    operation: PRESENT_OPERATION,
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none() =>
            {
                self.state = SerialOperationState::Complete;
                OperationAction::Complete
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                OperationAction::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::Cancelled,
                    detail: 21,
                })
            }
            OperationInput::Closed { port: PortId(0) }
                if self.state == SerialOperationState::Complete =>
            {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostOperationFailed,
                detail: 22,
            }),
        }
    }

    fn cancel(&mut self) {
        self.state = SerialOperationState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileOperation {
    Timer(TimerOperation),
    Serial(SerialOperation),
}

impl Operation for ProfileOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Timer(operation) => operation.start(),
            Self::Serial(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Timer(operation) => operation.resume(input),
            Self::Serial(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Timer(operation) => operation.advance(),
            Self::Serial(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Timer(operation) => operation.cancel(),
            Self::Serial(operation) => operation.cancel(),
        }
    }
}

pub struct KernelProfile {
    scheduler: Scheduler,
}

impl KernelProfile {
    pub fn new() -> Result<Self, SchedulerError> {
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let timer_value = conduit_kernel::ValueStorage::store(&mut values, TIMER_VALUE)?;
        let timer_wait = conduit_kernel::ValueStorage::store(&mut values, TIMER_WAIT)?;

        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(NODE_COUNT as u16);
        routes.install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )?;
        routes.seal()?;

        let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(NODE_COUNT as u16);
        bindings.install(
            NodeId(0),
            HostOperationBinding {
                operation: WAIT_OPERATION,
                maximum_input_bytes: 16,
                maximum_output_bytes: 16,
            },
        )?;
        bindings.install(
            NodeId(1),
            HostOperationBinding {
                operation: PRESENT_OPERATION,
                maximum_input_bytes: 16,
                maximum_output_bytes: 0,
            },
        )?;
        bindings.seal()?;

        let nodes = [
            NodeSpec {
                input_cords: [None; PORTS],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: {
                    let mut cords = [None; PORTS];
                    cords[0] = Some(CordId(0));
                    cords
                },
                maximum_step_work: 2,
            },
        ];
        let cords = [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 16,
            },
        )];
        let drivers = [
            OperationDriver::new(ProfileOperation::Timer(TimerOperation::new(
                timer_wait,
                timer_value,
            )?))?,
            OperationDriver::new(ProfileOperation::Serial(SerialOperation {
                state: SerialOperationState::Waiting,
            }))?,
        ];
        let sign_bytes = u32::try_from(SIGN_CAPACITY * core::mem::size_of::<KernelEvent>())
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(sign_bytes)?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
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

    pub fn timer_interest(request: HostOperationRequest) -> Result<KernelInterest, SchedulerError> {
        if request.node != NodeId(0)
            || request.request != TIMER_REQUEST
            || request.operation != WAIT_OPERATION
        {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        Ok(KernelInterest {
            node: request.node,
            request: request.request,
            input: request.input,
        })
    }

    pub fn complete_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_operation(
            interest.node,
            interest.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    pub fn fail_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_operation(
            interest.node,
            interest.request,
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

    pub fn complete_serial(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        if request.node != NodeId(1)
            || request.request != PRESENT_REQUEST
            || request.operation != PRESENT_OPERATION
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

#[cfg(test)]
mod tests;
