//! Fixed, allocation-free standing timer execution; no Root or provider access.
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallOutcome, KernelEvent, NodeId, PortId, RequestId, SignSink,
    ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostCallRequest, SchedulerError, SchedulerStatus, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
};

#[path = "tour_timer_runtime/preparation.rs"]
mod preparation;
#[path = "tour_timer_runtime/wire.rs"]
mod wire;
pub(crate) use preparation::{PreparedTimerGraph, PreparedTimerRoute};

pub(crate) const NODES: usize = 3;
pub(crate) const CORDS: usize = 2;
// Every selected timer/count/presentation Back has exactly one port ordinal.
pub(crate) const PORTS: usize = 1;
pub(crate) const HOST_BINDINGS: usize = NODES * NODES;
pub(crate) const VALUES: usize = 6;
pub(crate) const VALUE_BYTES: usize = 48;
pub(crate) const SIGNS: usize = 96;

pub(crate) type Scheduler = FixedScheduler<
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
pub(crate) enum TimerBack {
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

impl StepBack<PORTS> for TimerBack {
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
    let Ok(input) = BoundedValueRef::new(value, core::mem::size_of::<u64>() as u32) else {
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
    pub(crate) fn from_prepared_graph(graph: PreparedTimerGraph) -> Result<Self, SchedulerError> {
        let indices = [graph.timer.0, graph.count.0, graph.presentation.0];
        if indices.iter().any(|index| usize::from(*index) >= NODES)
            || graph.timer == graph.count
            || graph.timer == graph.presentation
            || graph.count == graph.presentation
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut values = FixedValueStore::<VALUES, 8>::new(VALUE_BYTES as u32)?;
        let wait = values.store(&graph.period.to_le_bytes())?;
        let next_wait = values.store(&graph.period.to_le_bytes())?;
        let tick = values.store(&conduit_time::encode_tick(0))?;
        let zero = values.store(&graph.start.to_le_bytes())?;
        let next = graph
            .start
            .checked_add(1)
            .ok_or(SchedulerError::InvalidPlan)?;
        let one = values.store(&next.to_le_bytes())?;
        let waits = [
            BoundedValueRef::new(wait, 8)?,
            BoundedValueRef::new(next_wait, 8)?,
        ];
        let drivers = core::array::from_fn(|index| {
            if index == usize::from(graph.timer.0) {
                TimerBack::Every {
                    waits,
                    tick,
                    emitting: false,
                    next_wait: 0,
                }
            } else if index == usize::from(graph.count.0) {
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
            }
        });
        let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
        for route in graph.routes.into_iter().flatten() {
            routes.install(route.node, route.port, route.range, &[route.target])?;
        }
        routes.seal()?;
        let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
        for (node, binding) in graph.bindings.into_iter().flatten() {
            bindings.install(node, binding)?;
        }
        bindings.seal()?;
        let minimum_sign_bytes = (SIGNS * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGNS>::new(graph.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_calls(
                graph.nodes,
                graph.cords,
                routes,
                bindings,
                drivers,
                values,
                signs,
            )?,
            timer: graph.timer,
            presentation: graph.presentation,
        })
    }

    pub(crate) fn complete_request(
        &mut self,
        node: NodeId,
        request: RequestId,
    ) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_call(
            node,
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
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

#[cfg(test)]
#[path = "tour_timer_runtime/tests.rs"]
mod tests;
