//! Fixed native kernel for the standing `count-over-time` Tour Form.

use crate::machine::KernelInterest;
use alloc::vec::Vec;
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, NodeId, Operation,
    OperationAction, OperationInput, PortId, RequestId, SignSink, ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};

const NODES: usize = 3;
const CORDS: usize = 2;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 6;
const VALUE_BYTES: usize = 48;
const SIGNS: usize = 96;

type Driver = OperationDriver<TimerOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<VALUES, 8>,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    { NODES * PORTS },
    CORDS,
    HOST_BINDINGS,
    2,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimerOperation {
    Every {
        waits: [BoundedValueRef; 2],
        tick: ValueRef,
        emitting: bool,
        next_wait: usize,
    },
    Count {
        zero: ValueRef,
        one: ValueRef,
        initial_emitted: bool,
        bumped: bool,
    },
    Presentation {
        pending: Option<RequestId>,
        next_request: u32,
    },
}

impl Operation for TimerOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Every {
                waits, next_wait, ..
            } => request_wait(waits, next_wait),
            Self::Count { zero, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *zero,
            },
            Self::Presentation { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Every {
                    tick,
                    emitting,
                    next_wait,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if usize::try_from(request.0).ok() == Some(*next_wait)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *emitting = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: *tick,
                }
            }
            (
                Self::Count {
                    one,
                    initial_emitted: true,
                    bumped,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*bumped && value.byte_len == conduit_time::TICK_ENCODED_LEN => {
                *bumped = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: *one,
                }
            }
            (
                Self::Presentation {
                    pending,
                    next_request,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() => {
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_semantic_catalog::COUNT_ENCODED_LEN)
                else {
                    return invalid(20);
                };
                let request = RequestId(*next_request);
                *next_request = next_request.saturating_add(1);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            (
                Self::Presentation { pending, .. },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                OperationAction::Await
            }
            _ => invalid(21),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Every {
                waits,
                emitting,
                next_wait,
                ..
            } if *emitting => {
                *emitting = false;
                request_wait(waits, next_wait)
            }
            Self::Count {
                initial_emitted, ..
            } => {
                *initial_emitted = true;
                OperationAction::Await
            }
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        if let Self::Presentation { pending, .. } = self {
            *pending = None;
        }
    }
}

fn request_wait(waits: &[BoundedValueRef; 2], next_wait: &mut usize) -> OperationAction {
    let Some(wait) = waits.get(*next_wait).copied() else {
        return invalid(10);
    };
    let request = RequestId(u32::try_from(*next_wait + 1).unwrap_or(u32::MAX));
    *next_wait += 1;
    OperationAction::RequestHostOperation {
        request,
        operation: conduit_kernel::HostOperationId(0),
        input: wait,
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

pub struct TourTimerKernel {
    scheduler: Scheduler,
    timer: NodeId,
    presentation: NodeId,
}

impl TourTimerKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        if fragment.placements.len() != NODES
            || fragment.connections.len() != CORDS
            || lowered.nodes.len() != NODES
            || lowered.cords.len() != CORDS
            || !lowered.remote_endpoints.is_empty()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let timer = node(fragment, conduit_time::TIME_EVERY_KIND)?;
        let count = node(fragment, conduit_semantic_catalog::STATE_COUNT_KIND)?;
        let presentation = node(fragment, conduit_semantic_catalog::COUNT_PRESENTATION_KIND)?;
        validate_implementation(fragment, timer, crate::offer::TIME_EVERY_IMPLEMENTATION)?;
        validate_implementation(fragment, count, crate::offer::STATE_COUNT_IMPLEMENTATION)?;
        validate_implementation(
            fragment,
            presentation,
            crate::offer::COUNT_PRESENTATION_IMPLEMENTATION,
        )?;
        let period = configured_milliseconds(&fragment.placements[timer].configuration, "freq")?;
        let start = configured_u64(&fragment.placements[count].configuration, "start")?;
        if period != 120 || start != 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut values = FixedValueStore::<VALUES, 8>::new(VALUE_BYTES as u32)?;
        let wait = values.store(&period.to_le_bytes())?;
        let next_wait = values.store(&period.to_le_bytes())?;
        let tick = values.store(&conduit_time::encode_tick(0))?;
        let zero = values.store(&start.to_le_bytes())?;
        let one = values.store(&1_u64.to_le_bytes())?;
        let mut drivers = Vec::with_capacity(NODES);
        for index in 0..NODES {
            let operation = if index == timer {
                TimerOperation::Every {
                    waits: [
                        BoundedValueRef::new(wait, 8)?,
                        BoundedValueRef::new(next_wait, 8)?,
                    ],
                    tick,
                    emitting: false,
                    next_wait: 0,
                }
            } else if index == count {
                TimerOperation::Count {
                    zero,
                    one,
                    initial_emitted: false,
                    bumped: false,
                }
            } else {
                TimerOperation::Presentation {
                    pending: None,
                    next_request: 1,
                }
            };
            drivers.push(OperationDriver::new(operation)?);
        }
        let drivers: [Driver; NODES] = drivers
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
        let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes.install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
        for operation in &lowered.host_operations {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;
        let minimum_sign_bytes = (SIGNS * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGNS>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            timer: NodeId(timer as u16),
            presentation: NodeId(presentation as u16),
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

    pub fn is_timer(&self, request: &HostOperationRequest) -> bool {
        request.node == self.timer
    }

    pub fn is_presentation(&self, request: &HostOperationRequest) -> bool {
        request.node == self.presentation
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

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        self.complete(request)
    }

    fn complete(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
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

    pub fn pending_host_operations(&self) -> usize {
        self.scheduler.pending_host_operation_count()
    }

    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }

    pub fn sign_count(&self) -> u16 {
        self.scheduler.signs().len()
    }
}

fn node(fragment: &PlanFragment, kind: &str) -> Result<usize, SchedulerError> {
    fragment
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == kind)
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_implementation(
    fragment: &PlanFragment,
    node: usize,
    implementation: &str,
) -> Result<(), SchedulerError> {
    if fragment.placements[node].implementation_id.as_str() != implementation {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
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

fn configured_milliseconds(
    entries: &[conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<u64, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::Quantity(value)) if candidate == key => value
                .convert(conduit_core::QuantityUnit::Millisecond)
                .ok()
                .and_then(|value| u64::try_from(value.value()).ok()),
            _ => None,
        })
        .ok_or(SchedulerError::InvalidPlan)
}
