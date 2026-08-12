//! Production-kernel execution of one planned monophonic tone sink.

use conduit_core::{Gate, ToneIntent};
use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, KernelEvent, NodeId, Operation, OperationAction, OperationInput, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueRef, ValueStorage,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver,
        SchedulerError, SchedulerStatus,
    },
};

use crate::{
    machine::{RealizedTone, ToneBase},
    ordinary_plan::PreparationError,
    pc_speaker_plan::PreparedPcSpeakerPlay,
};

const SOURCE_NODE: NodeId = NodeId(0);
const SINK_NODE: NodeId = NodeId(1);
const FIXTURE_OPERATION: HostOperationId = HostOperationId(0);
const TONE_OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 1;
const EVENTS: usize = 4;
const SIGN_CAPACITY: usize = 64;

#[derive(Clone, Copy)]
struct SourceOperation {
    values: [ValueRef; EVENTS],
    tokens: [ValueRef; EVENTS],
    next: usize,
}

impl Operation for SourceOperation {
    fn start(&mut self) -> OperationAction {
        self.request_or_complete()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if request == RequestId((self.next + 1) as u32)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                OperationAction::Emit {
                    port: PortId(0),
                    value: self.values[self.next],
                }
            }
            _ => invalid(10),
        }
    }

    fn advance(&mut self) -> OperationAction {
        self.next += 1;
        self.request_or_complete()
    }

    fn cancel(&mut self) {
        self.next = EVENTS;
    }
}

impl SourceOperation {
    fn request_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .map_or(OperationAction::Complete, |_value| {
                OperationAction::RequestHostOperation {
                    request: RequestId((self.next + 1) as u32),
                    operation: FIXTURE_OPERATION,
                    input: BoundedValueRef::new(self.tokens[self.next], 8)
                        .expect("fixture token is exactly admitted"),
                }
            })
    }
}

#[derive(Clone, Copy)]
struct ToneOperation {
    pending: Option<RequestId>,
    next_request: u32,
}

impl Operation for ToneOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_core::TONE_INTENT_ENCODED_LEN as u32)
                else {
                    return invalid(20);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: TONE_OPERATION,
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => invalid(21),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
enum PcSpeakerOperation {
    Source(SourceOperation),
    Tone(ToneOperation),
}

impl Operation for PcSpeakerOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(operation) => operation.start(),
            Self::Tone(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(operation) => operation.resume(input),
            Self::Tone(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(operation) => operation.advance(),
            Self::Tone(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Source(operation) => operation.cancel(),
            Self::Tone(operation) => operation.cancel(),
        }
    }
}

type Driver = OperationDriver<PcSpeakerOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<{ EVENTS * 2 }, 256>,
    FixedSignLog<SIGN_CAPACITY>,
    2,
    1,
    PORTS,
    1,
    1,
    1,
    2,
    2,
>;

pub struct PcSpeakerKernel {
    scheduler: Scheduler,
}

pub struct PreparedPcSpeakerExecution {
    kernel: PcSpeakerKernel,
}

impl PcSpeakerKernel {
    fn prepare(values: [ToneIntent; EVENTS]) -> Result<Self, SchedulerError> {
        let mut store = FixedValueStore::<{ EVENTS * 2 }, 256>::new(256)?;
        let references = values
            .map(|value| store.store(&value.encode()))
            .into_iter()
            .collect::<Result<alloc::vec::Vec<_>, _>>()?
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let token_bytes: [[u8; 8]; EVENTS] =
            core::array::from_fn(|index| (index as u64).to_le_bytes());
        let tokens: [ValueRef; EVENTS] = token_bytes
            .map(|token| store.store(&token))
            .into_iter()
            .collect::<Result<alloc::vec::Vec<_>, _>>()?
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        routes.install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(SINK_NODE, PortId(0)),
            }],
        )?;
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<2>::new(1);
        bindings.install(
            SOURCE_NODE,
            HostOperationBinding {
                operation: FIXTURE_OPERATION,
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )?;
        bindings.install(
            SINK_NODE,
            HostOperationBinding {
                operation: TONE_OPERATION,
                maximum_input_bytes: conduit_core::TONE_INTENT_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            },
        )?;
        bindings.seal()?;
        let signs =
            FixedSignLog::new((SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32)?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                [
                    NodeSpec {
                        input_cords: [None],
                        maximum_step_work: 4,
                    },
                    NodeSpec {
                        input_cords: [Some(CordId(0))],
                        maximum_step_work: 4,
                    },
                ],
                [CordSpec::local(
                    CordId(0),
                    (SOURCE_NODE, PortId(0)),
                    (SINK_NODE, PortId(0)),
                    CordCapacity {
                        slot_start: 0,
                        item_capacity: 1,
                        byte_capacity: conduit_core::TONE_INTENT_ENCODED_LEN as u32,
                    },
                )],
                routes,
                bindings,
                [
                    OperationDriver::new(PcSpeakerOperation::Source(SourceOperation {
                        values: references,
                        tokens,
                        next: 0,
                    }))?,
                    OperationDriver::new(PcSpeakerOperation::Tone(ToneOperation {
                        pending: None,
                        next_request: 1,
                    }))?,
                ],
                store,
                signs,
            )?,
        })
    }

    fn next_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }

    fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(value)
    }

    fn complete(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        let expected = match request.node {
            SOURCE_NODE => FIXTURE_OPERATION,
            SINK_NODE => TONE_OPERATION,
            _ => return Err(SchedulerError::InvalidHostOperationAccess),
        };
        if request.operation != expected {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSpeakerPlayReport {
    pub realized: [RealizedTone; EVENTS],
    pub transitions: u32,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub final_gate_open: bool,
    pub completed: bool,
}

pub fn prepare_execution(
    prepared: &PreparedPcSpeakerPlay,
    values: [ToneIntent; EVENTS],
) -> Result<PreparedPcSpeakerExecution, PreparationError> {
    validate_prepared(prepared)?;
    Ok(PreparedPcSpeakerExecution {
        kernel: PcSpeakerKernel::prepare(values).map_err(|_| PreparationError::KernelRejected)?,
    })
}

pub fn run<B: ToneBase>(
    execution: &mut PreparedPcSpeakerExecution,
    base: &mut B,
) -> Result<PcSpeakerPlayReport, PreparationError> {
    let kernel = &mut execution.kernel;
    let mut realized = [RealizedTone {
        correlation: 0,
        requested_millihertz: 0,
        realized_millihertz: 0,
        divisor: 0,
        gate_open: false,
    }; EVENTS];
    let mut realized_count = 0;
    for _ in 0..128 {
        let status = kernel
            .scheduler
            .step()
            .map_err(|_| PreparationError::KernelRejected)?;
        while let Some(request) = kernel.next_request() {
            if request.node == SOURCE_NODE {
                kernel
                    .complete(request)
                    .map_err(|_| PreparationError::KernelRejected)?;
                continue;
            }
            let encoded = kernel
                .host_value(request.input.value)
                .map_err(|_| PreparationError::KernelRejected)?;
            let intent =
                ToneIntent::decode(encoded).map_err(|_| PreparationError::KernelRejected)?;
            let outcome = base.apply(intent).map_err(|_| {
                let _ = base.silence();
                PreparationError::KernelRejected
            })?;
            let Some(slot) = realized.get_mut(realized_count) else {
                let _ = base.silence();
                return Err(PreparationError::KernelRejected);
            };
            *slot = outcome;
            realized_count += 1;
            kernel
                .complete(request)
                .map_err(|_| PreparationError::KernelRejected)?;
        }
        if matches!(status, SchedulerStatus::Complete) {
            if realized_count != EVENTS || realized[EVENTS - 1].gate_open {
                let _ = base.silence();
                return Err(PreparationError::KernelRejected);
            }
            return Ok(PcSpeakerPlayReport {
                realized,
                transitions: base.transition_count(),
                kernel_decisions: kernel.scheduler.decisions(),
                kernel_signs: kernel.scheduler.signs().len(),
                final_gate_open: false,
                completed: true,
            });
        }
    }
    let _ = base.silence();
    Err(PreparationError::KernelRejected)
}

fn validate_prepared(prepared: &PreparedPcSpeakerPlay) -> Result<(), PreparationError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(PreparationError::PlanRejected)?;
    if fragment.placements.len() != 2
        || fragment.connections.len() != 1
        || !fragment.placements.iter().any(|placement| {
            placement.kind_id.as_str() == conduit_std_catalog::SOUND_TONE_PLAY_KIND
        })
        || prepared.active_play.plan_id != prepared.plan.plan_id
        || prepared.active_play.host_id != fragment.host_id
        || prepared.active_play.boot_id != fragment.boot_id
        || prepared.active_play
            != conduit_core::bind_active_play(
                &prepared.plan.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                prepared.active_play.play_sequence,
            )
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

pub fn reviewed_values() -> [ToneIntent; EVENTS] {
    [
        tone(1, 440_000, Gate::On, 0),
        tone(1, 440_000, Gate::Off, 1),
        tone(2, 660_000, Gate::On, 2),
        tone(2, 660_000, Gate::Off, 3),
    ]
}

fn tone(correlation: u64, frequency: u64, gate: Gate, order: u32) -> ToneIntent {
    ToneIntent::new(
        correlation,
        conduit_core::MusicalPitch::new(frequency, 440_000, 0).expect("reviewed pitch"),
        gate,
        u64::from(order) * 1_000,
        order,
    )
    .expect("reviewed tone")
}

#[cfg(test)]
#[path = "pc_speaker_play_tests.rs"]
mod tests;
