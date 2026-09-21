//! Fixed native kernel for the standing `count-over-time` Tour Form.

use crate::machine::KernelInterest;
use alloc::vec::Vec;
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallOutcome, KernelEvent, NodeId, PortId, RequestId, SignSink,
    ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostCallRequest, SchedulerError, SchedulerStatus, StepInputBytes, StepIo,
        StepOperation, StepOutcome,
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

type Scheduler = FixedScheduler<
    TimerBack,
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
enum TimerBack {
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

impl StepOperation<PORTS> for TimerBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Every {
                waits,
                tick,
                emitting,
                next_wait,
            } => step_every(waits, *tick, emitting, next_wait, io),
            Self::Count {
                zero,
                one,
                initial_emitted,
                bumped,
            } => {
                if !*initial_emitted {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), *zero).is_err() {
                        return invalid(21);
                    }
                    *initial_emitted = true;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if *bumped || value.byte_len != conduit_time::TICK_ENCODED_LEN {
                        return invalid(21);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.consume(PortId(0)).is_err() || io.send(PortId(0), *one).is_err() {
                        return invalid(21);
                    }
                    *bumped = true;
                    return StepOutcome::Progress;
                }
                StepOutcome::Await
            }
            Self::Presentation {
                pending,
                next_request,
            } => step_presentation(pending, next_request, io),
        }
    }

    fn cancel(&mut self) {
        if let Self::Presentation { pending, .. } = self {
            *pending = None;
        }
    }
}

fn step_every(
    waits: &[BoundedValueRef; 2],
    tick: ValueRef,
    pending: &mut bool,
    next_wait: &mut usize,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if usize::try_from(request.0).ok() != Some(*next_wait)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
        {
            return invalid(21);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume_host_completion().is_err() || io.send(PortId(0), tick).is_err() {
            return invalid(21);
        }
        *pending = false;
        return StepOutcome::Progress;
    }
    let Some(wait) = waits.get(*next_wait).copied() else {
        return invalid(10);
    };
    let request = RequestId(u32::try_from(*next_wait + 1).unwrap_or(u32::MAX));
    if io
        .request_host_call(request, conduit_kernel::HostCallId(0), wait)
        .is_err()
    {
        return invalid(10);
    }
    *next_wait += 1;
    *pending = true;
    StepOutcome::Progress
}

fn step_presentation(
    pending: &mut Option<RequestId>,
    next_request: &mut u32,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if let Some(request) = *pending {
        let Some((completed, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if completed != request
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(21);
        }
        *pending = None;
        return StepOutcome::Progress;
    }
    let Some(value) = io.input(PortId(0)) else {
        return StepOutcome::Await;
    };
    let Ok(input) = BoundedValueRef::new(value, conduit_semantic_catalog::COUNT_ENCODED_LEN) else {
        return invalid(20);
    };
    let request = RequestId(*next_request);
    if io.consume(PortId(0)).is_err()
        || io
            .request_host_call(request, conduit_kernel::HostCallId(0), input)
            .is_err()
    {
        return invalid(21);
    }
    *next_request = next_request.saturating_add(1);
    *pending = Some(request);
    StepOutcome::Progress
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
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
                TimerBack::Every {
                    waits: [
                        BoundedValueRef::new(wait, 8)?,
                        BoundedValueRef::new(next_wait, 8)?,
                    ],
                    tick,
                    emitting: false,
                    next_wait: 0,
                }
            } else if index == count {
                TimerBack::Count {
                    zero,
                    one,
                    initial_emitted: false,
                    bumped: false,
                }
            } else {
                TimerBack::Presentation {
                    pending: None,
                    next_request: 1,
                }
            };
            drivers.push(operation);
        }
        let drivers: [TimerBack; NODES] = drivers
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
        let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
        for operation in &lowered.host_calls {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;
        let minimum_sign_bytes = (SIGNS * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGNS>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_calls(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            timer: NodeId(timer as u16),
            presentation: NodeId(presentation as u16),
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

    pub fn is_timer(&self, request: &HostCallRequest) -> bool {
        request.node == self.timer
    }

    pub fn is_presentation(&self, request: &HostCallRequest) -> bool {
        request.node == self.presentation
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

    pub fn complete_presentation(
        &mut self,
        request: HostCallRequest,
    ) -> Result<(), SchedulerError> {
        self.complete(request)
    }

    fn complete(&mut self, request: HostCallRequest) -> Result<(), SchedulerError> {
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

    pub fn pending_host_calls(&self) -> usize {
        self.scheduler.pending_host_call_count()
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
