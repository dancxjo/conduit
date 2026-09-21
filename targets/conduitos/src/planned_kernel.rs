//! Allocation-independent installation of one already lowered ordinary plan.

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

use crate::machine::KernelInterest;

pub const TIMER_NODE: NodeId = NodeId(0);
pub const PRESENT_NODE: NodeId = NodeId(1);
const TIMER_REQUEST: RequestId = RequestId(1);
const PRESENT_REQUEST: RequestId = RequestId(2);
const MAX_NODES: usize = 2;
const MAX_CORDS: usize = 1;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 1;
const ROUTE_SLOTS: usize = 1;
const ROUTE_TARGETS: usize = 1;
const HOST_BINDING_SLOTS: usize = 4;
const PENDING_REQUESTS: usize = 2;
const VALUE_SLOTS: usize = 4;
const VALUE_BYTES: usize = 64;
const SIGN_CAPACITY: usize = 64;

type Scheduler = FixedScheduler<
    PlannedBack,
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
enum TimerState {
    Waiting,
    Requested,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerBack {
    wait: BoundedValueRef,
    tick: ValueRef,
    state: TimerState,
}

impl TimerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match self.state {
            TimerState::Waiting => {
                if io
                    .request_host_call(TIMER_REQUEST, conduit_kernel::HostCallId(0), self.wait)
                    .is_err()
                {
                    return invalid(11);
                }
                self.state = TimerState::Requested;
                StepOutcome::Progress
            }
            TimerState::Requested => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request == TIMER_REQUEST && outcome.disposition == HostCallDisposition::Cancelled
                {
                    return failure(conduit_kernel::FailureCode::Cancelled, 10);
                }
                if request != TIMER_REQUEST
                    || outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                {
                    return failure(conduit_kernel::FailureCode::HostCallFailed, 11);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err() || io.send(PortId(0), self.tick).is_err() {
                    return invalid(11);
                }
                self.state = TimerState::Complete;
                StepOutcome::Complete
            }
            TimerState::Complete => StepOutcome::Complete,
            TimerState::Cancelled => failure(conduit_kernel::FailureCode::Cancelled, 10),
        }
    }

    fn cancel(&mut self) {
        self.state = TimerState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PresentationBack {
    pending: bool,
    complete: bool,
}

impl PresentationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != PRESENT_REQUEST
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
                || io.consume_host_completion().is_err()
            {
                return invalid(21);
            }
            self.pending = false;
            self.complete = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) = BoundedValueRef::new(value, 8) else {
                return invalid(20);
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(PRESENT_REQUEST, conduit_kernel::HostCallId(0), input)
                    .is_err()
            {
                return invalid(21);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        if self.complete && io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return invalid(21);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn invalid(detail: u16) -> StepOutcome {
    failure(conduit_kernel::FailureCode::InvalidLifecycle, detail)
}

const fn failure(code: conduit_kernel::FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure { code, detail })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedBack {
    Timer(TimerBack),
    Presentation(PresentationBack),
}

impl StepOperation<PORTS> for PlannedBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Timer(back) => back.step(io),
            Self::Presentation(back) => back.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Timer(operation) => operation.cancel(),
            Self::Presentation(operation) => operation.cancel(),
        }
    }
}

pub struct PlannedKernel {
    scheduler: Scheduler,
}

impl PlannedKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        validate_shape(fragment, lowered)?;
        let period_ms = configured_u64(&fragment.placements[0].configuration, "period-ms")?;
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let wait = values.store(&period_ms.to_le_bytes())?;
        let tick = values.store(&0_u64.to_le_bytes())?;
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
        let mut bindings = FixedHostCallBindings::<HOST_BINDING_SLOTS>::new(MAX_NODES as u16);
        for operation in &lowered.host_calls {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;
        let drivers = [
            PlannedBack::Timer(TimerBack {
                wait: BoundedValueRef::new(wait, 8)?,
                tick,
                state: TimerState::Waiting,
            }),
            PlannedBack::Presentation(PresentationBack {
                pending: false,
                complete: false,
            }),
        ];
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
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
        if request.node != TIMER_NODE || request.call != conduit_kernel::HostCallId(0) {
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

    #[cfg(test)]
    fn fail_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
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

    pub fn complete_presentation(
        &mut self,
        request: HostCallRequest,
    ) -> Result<(), SchedulerError> {
        if request.node != PRESENT_NODE || request.call != conduit_kernel::HostCallId(0) {
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

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<(), SchedulerError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.routes.len() != ROUTE_SLOTS
        || lowered.host_calls.len() != 2
        || lowered.cord_value_slots != 1
        || lowered.cord_value_bytes != 8
        || fragment.placements[0].kind_id.as_str() != conduit_semantic_catalog::TICK_KIND
        || fragment.placements[1].kind_id.as_str()
            != conduit_semantic_catalog::TICK_PRESENTATION_KIND
        || fragment.placements[0].implementation_id.as_str()
            != crate::offer::TIME_TICK_IMPLEMENTATION
        || fragment.placements[1].implementation_id.as_str()
            != crate::offer::TICK_PRESENTATION_IMPLEMENTATION
        || configured_u64(&fragment.placements[0].configuration, "count")? != 1
        || !lowered.remote_endpoints.is_empty()
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
        timing_plan,
    };

    fn kernel() -> PlannedKernel {
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
        timing_plan::prepare_timing(&identities, &offer, "build")
            .unwrap()
            .kernel
    }

    fn timer_interest(kernel: &mut PlannedKernel) -> KernelInterest {
        assert!(matches!(
            kernel.step(),
            Ok(SchedulerStatus::Progress { .. })
        ));
        PlannedKernel::timer_interest(kernel.next_host_request().unwrap()).unwrap()
    }

    #[test]
    fn ordinary_cancellation_rejects_a_late_timer_wake() {
        let mut kernel = kernel();
        let interest = timer_interest(&mut kernel);
        kernel.cancel().unwrap();
        assert_eq!(
            kernel.complete_timer(interest),
            Err(SchedulerError::HostCallCompletionRejected)
        );
        assert_eq!(kernel.step(), Ok(SchedulerStatus::Cancelled));
    }

    #[test]
    fn ordinary_timer_base_loss_remains_terminal_failure() {
        let mut kernel = kernel();
        let interest = timer_interest(&mut kernel);
        kernel.fail_timer(interest).unwrap();
        assert!(matches!(
            kernel.step(),
            Ok(SchedulerStatus::Progress { .. })
        ));
        assert_eq!(
            kernel.step(),
            Err(SchedulerError::BackFailed(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 11
            }))
        );
    }
}
