//! Fenced P2/P3 hand-lowered regression fixture; production uses `planned_kernel`.

use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, KernelEvent, NodeId, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueRef,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostCallRequest, NodeSpec, SchedulerError,
        SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
};

use crate::machine::KernelInterest;

pub const WAIT_OPERATION: HostCallId = HostCallId(0);
pub const PRESENT_OPERATION: HostCallId = HostCallId(0);
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

type Scheduler = FixedScheduler<
    ProfileBack,
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
enum TimerBackState {
    Waiting,
    Requested,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerBack {
    wait: BoundedValueRef,
    tick: ValueRef,
    state: TimerBackState,
}

impl TimerBack {
    fn new(wait: ValueRef, tick: ValueRef) -> Result<Self, SchedulerError> {
        Ok(Self {
            wait: BoundedValueRef::new(wait, 8)?,
            tick,
            state: TimerBackState::Waiting,
        })
    }
}

impl TimerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match self.state {
            TimerBackState::Waiting => {
                if io
                    .request_host_call(TIMER_REQUEST, WAIT_OPERATION, self.wait)
                    .is_err()
                {
                    return fail(conduit_kernel::FailureCode::InvalidInput, 12);
                }
                self.state = TimerBackState::Requested;
                StepOutcome::Progress
            }
            TimerBackState::Requested => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != TIMER_REQUEST {
                    return fail(conduit_kernel::FailureCode::InvalidInput, 12);
                }
                if outcome.disposition == HostCallDisposition::Cancelled {
                    return fail(conduit_kernel::FailureCode::Cancelled, 10);
                }
                if outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                {
                    return fail(conduit_kernel::FailureCode::HostCallFailed, 11);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err() || io.send(PortId(0), self.tick).is_err() {
                    return fail(conduit_kernel::FailureCode::InvalidInput, 12);
                }
                self.state = TimerBackState::Complete;
                StepOutcome::Complete
            }
            TimerBackState::Complete => StepOutcome::Complete,
            TimerBackState::Cancelled => fail(conduit_kernel::FailureCode::Cancelled, 10),
        }
    }

    fn cancel(&mut self) {
        self.state = TimerBackState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SerialBackState {
    Waiting,
    Presenting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SerialBack {
    state: SerialBackState,
}

impl SerialBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match self.state {
            SerialBackState::Waiting => {
                let Some(value) = io.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                let Ok(input) = BoundedValueRef::new(value, 16) else {
                    return fail(conduit_kernel::FailureCode::InvalidInput, 20);
                };
                if io.consume(PortId(0)).is_err()
                    || io
                        .request_host_call(PRESENT_REQUEST, PRESENT_OPERATION, input)
                        .is_err()
                {
                    return fail(conduit_kernel::FailureCode::HostCallFailed, 22);
                }
                self.state = SerialBackState::Presenting;
                StepOutcome::Progress
            }
            SerialBackState::Presenting => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != PRESENT_REQUEST {
                    return fail(conduit_kernel::FailureCode::HostCallFailed, 22);
                }
                if outcome.disposition == HostCallDisposition::Cancelled {
                    return fail(conduit_kernel::FailureCode::Cancelled, 21);
                }
                if outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                    || io.consume_host_completion().is_err()
                {
                    return fail(conduit_kernel::FailureCode::HostCallFailed, 22);
                }
                self.state = SerialBackState::Complete;
                StepOutcome::Complete
            }
            SerialBackState::Complete => StepOutcome::Complete,
            SerialBackState::Cancelled => fail(conduit_kernel::FailureCode::Cancelled, 21),
        }
    }

    fn cancel(&mut self) {
        self.state = SerialBackState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileBack {
    Timer(TimerBack),
    Serial(SerialBack),
}

impl StepBack<PORTS> for ProfileBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Timer(back) => back.step(io),
            Self::Serial(back) => back.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Timer(operation) => operation.cancel(),
            Self::Serial(operation) => operation.cancel(),
        }
    }
}

const fn fail(code: conduit_kernel::FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure { code, detail })
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

        let mut bindings = FixedHostCallBindings::<HOST_BINDING_SLOTS>::new(NODE_COUNT as u16);
        bindings.install(
            NodeId(0),
            HostCallBinding {
                call: WAIT_OPERATION,
                maximum_input_bytes: 16,
                maximum_output_bytes: 16,
            },
        )?;
        bindings.install(
            NodeId(1),
            HostCallBinding {
                call: PRESENT_OPERATION,
                maximum_input_bytes: 16,
                maximum_output_bytes: 0,
            },
        )?;
        bindings.seal()?;

        let nodes = [
            NodeSpec {
                input_cords: [None; PORTS],
                maximum_step_fuel: 2,
            },
            NodeSpec {
                input_cords: {
                    let mut cords = [None; PORTS];
                    cords[0] = Some(CordId(0));
                    cords
                },
                maximum_step_fuel: 2,
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
                pressure_policy: Default::default(),
            },
        )];
        let drivers = [
            ProfileBack::Timer(TimerBack::new(timer_wait, timer_value)?),
            ProfileBack::Serial(SerialBack {
                state: SerialBackState::Waiting,
            }),
        ];
        let sign_bytes = u32::try_from(SIGN_CAPACITY * core::mem::size_of::<KernelEvent>())
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(sign_bytes)?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_calls(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.scheduler.next_host_request()
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(value)
    }

    pub fn timer_interest(request: HostCallRequest) -> Result<KernelInterest, SchedulerError> {
        if request.node != NodeId(0)
            || request.request != TIMER_REQUEST
            || request.call != WAIT_OPERATION
        {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        Ok(KernelInterest {
            node: request.node,
            request: request.request,
            input: request.input,
        })
    }

    pub fn complete_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_call(
            interest.node,
            interest.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    pub fn fail_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_call(
            interest.node,
            interest.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostCallFailed,
                    detail: 1,
                }),
            },
        )
    }

    pub fn complete_serial(&mut self, request: HostCallRequest) -> Result<(), SchedulerError> {
        if request.node != NodeId(1)
            || request.request != PRESENT_REQUEST
            || request.call != PRESENT_OPERATION
        {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        self.scheduler.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
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

    pub fn pending_host_calls(&self) -> usize {
        self.scheduler.pending_host_call_count()
    }
}

#[cfg(test)]
mod tests;
