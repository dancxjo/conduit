//! Production-kernel execution of one planned direct OPL2 musical sink.
use alloc::vec::Vec;
use conduit_audio::{Gate, MusicalNoteEvent, NoteOccurrenceId};

use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, KernelEvent, NodeId, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueRef, ValueStorage,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostCallRequest, NodeSpec, SchedulerStatus,
        StepBack, StepInputBytes, StepIo, StepOutcome,
    },
};
use conduit_semantic_catalog::{NormalizedNoteEvidence, SelectedSoundRealization};

use crate::{
    machine::{Opl2Base, Opl2Pitch},
    opl2_plan::PreparedOpl2Play,
    ordinary_plan::PreparationError,
};

const SOURCE_NODE: NodeId = NodeId(0);
const SINK_NODE: NodeId = NodeId(1);
const FIXTURE_OPERATION: HostCallId = HostCallId(0);
const OPL2_OPERATION: HostCallId = HostCallId(0);
const PORTS: usize = 1;
const EVENTS: usize = crate::opl2_plan::FIXTURE_EVENT_COUNT as usize;
const SIGN_CAPACITY: usize = 256;

#[derive(Clone, Copy)]
struct SourceBack {
    values: [ValueRef; EVENTS],
    tokens: [ValueRef; EVENTS],
    next: usize,
}

impl SourceBack {
    fn cancel(&mut self) {
        self.next = EVENTS;
    }

    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.next == EVENTS {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if request != RequestId((self.next + 1) as u32)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return invalid(10);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err()
                || io.send(PortId(0), self.values[self.next]).is_err()
            {
                return invalid(10);
            }
            self.next += 1;
            return if self.next == EVENTS {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        if io
            .request_host_call(
                RequestId((self.next + 1) as u32),
                FIXTURE_OPERATION,
                BoundedValueRef::new(self.tokens[self.next], 8)
                    .expect("fixture token is exactly admitted"),
            )
            .is_err()
        {
            return invalid(10);
        }
        StepOutcome::Progress
    }
}

#[derive(Clone, Copy)]
struct MusicBack {
    pending: Option<RequestId>,
    next_request: u32,
}

impl MusicBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(request) = self.pending {
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
            self.pending = None;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) =
                BoundedValueRef::new(value, conduit_audio::NOTE_EVENT_ENCODED_LEN as u32)
            else {
                return invalid(20);
            };
            let request = RequestId(self.next_request);
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(request, OPL2_OPERATION, input)
                    .is_err()
            {
                return invalid(21);
            }
            self.next_request = self.next_request.saturating_add(1);
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return invalid(21);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
#[expect(
    clippy::large_enum_variant,
    reason = "fixed pre-play fixture references avoid heap indirection in the freestanding kernel"
)]
enum Opl2Back {
    Source(SourceBack),
    Music(MusicBack),
}

impl StepBack<PORTS> for Opl2Back {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source(back) => back.step(io),
            Self::Music(back) => back.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Source(operation) => operation.cancel(),
            Self::Music(operation) => operation.cancel(),
        }
    }
}

type Scheduler = FixedScheduler<
    Opl2Back,
    FixedValueStore<{ EVENTS * 2 }, 1_536>,
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

pub struct PreparedOpl2Execution {
    scheduler: Scheduler,
    voices: [Option<Voice>; 9],
    register_writes: u16,
    peak_voices: u8,
    normalized_events: Vec<NormalizedNoteEvidence>,
    selected: Option<SelectedSoundRealization>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Voice {
    occurrence: NoteOccurrenceId,
    pitch: Opl2Pitch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Opl2PlayReport {
    pub events: u16,
    pub peak_voices: u8,
    pub reset_writes: u16,
    pub patch_writes: u16,
    pub event_writes: u16,
    pub quiesce_writes: u16,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub final_active_voices: u8,
    pub completed: bool,
}

pub fn prepare_execution(
    prepared: &PreparedOpl2Play,
    values: [MusicalNoteEvent; EVENTS],
) -> Result<PreparedOpl2Execution, PreparationError> {
    validate_prepared(prepared)?;
    let mut store = FixedValueStore::<{ EVENTS * 2 }, 1_536>::new(1_536)
        .map_err(|_| PreparationError::KernelRejected)?;
    let references = values
        .map(|value| store.store(&value.encode()))
        .into_iter()
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(|_| PreparationError::KernelRejected)?
        .try_into()
        .map_err(|_| PreparationError::KernelRejected)?;
    let token_bytes: [[u8; 8]; EVENTS] = core::array::from_fn(|index| (index as u64).to_le_bytes());
    let tokens = token_bytes
        .map(|token| store.store(&token))
        .into_iter()
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(|_| PreparationError::KernelRejected)?
        .try_into()
        .map_err(|_| PreparationError::KernelRejected)?;
    let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
    routes
        .install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(SINK_NODE, PortId(0)),
            }],
        )
        .map_err(|_| PreparationError::KernelRejected)?;
    routes
        .seal()
        .map_err(|_| PreparationError::KernelRejected)?;
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    bindings
        .install(
            SOURCE_NODE,
            HostCallBinding {
                call: FIXTURE_OPERATION,
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| PreparationError::KernelRejected)?;
    bindings
        .install(
            SINK_NODE,
            HostCallBinding {
                call: OPL2_OPERATION,
                maximum_input_bytes: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| PreparationError::KernelRejected)?;
    bindings
        .seal()
        .map_err(|_| PreparationError::KernelRejected)?;
    let signs = FixedSignLog::new((SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| PreparationError::KernelRejected)?;
    let scheduler = FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 4,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (SOURCE_NODE, PortId(0)),
            (SINK_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            Opl2Back::Source(SourceBack {
                values: references,
                tokens,
                next: 0,
            }),
            Opl2Back::Music(MusicBack {
                pending: None,
                next_request: 1,
            }),
        ],
        store,
        signs,
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    Ok(PreparedOpl2Execution {
        scheduler,
        voices: [None; 9],
        register_writes: 0,
        peak_voices: 0,
        normalized_events: Vec::with_capacity(EVENTS),
        selected: Some(SelectedSoundRealization {
            plan_id: prepared.plan.plan_id.clone(),
            host_id: prepared.active_play.host_id.clone(),
            boot_id: prepared.active_play.boot_id.clone(),
            implementation_id: conduit_core::ImplementationId::from(
                crate::opl2_offer::OPL2_IMPLEMENTATION,
            ),
        }),
    })
}

pub fn run<B: Opl2Base>(
    execution: &mut PreparedOpl2Execution,
    base: &mut B,
) -> Result<Opl2PlayReport, PreparationError> {
    let reset_writes = base.reset().map_err(|_| PreparationError::KernelRejected)?;
    execution.register_writes = reset_writes;
    let mut patch_writes = 0u16;
    for channel in 0..B::CHANNELS {
        patch_writes = patch_writes
            .checked_add(
                base.configure_fixed_patch(channel)
                    .map_err(|_| PreparationError::KernelRejected)?,
            )
            .ok_or(PreparationError::KernelRejected)?;
    }
    execution.register_writes = execution
        .register_writes
        .checked_add(patch_writes)
        .ok_or(PreparationError::KernelRejected)?;
    let mut event_writes = 0u16;
    let mut events = 0u16;
    for _ in 0..512 {
        let status = execution
            .scheduler
            .step()
            .map_err(|_| PreparationError::KernelRejected)?;
        while let Some(request) = execution.scheduler.next_host_request() {
            if request.node == SOURCE_NODE {
                complete(&mut execution.scheduler, request)?;
                continue;
            }
            let encoded = execution
                .scheduler
                .host_value(request.input.value)
                .map_err(|_| PreparationError::KernelRejected)?;
            let event =
                MusicalNoteEvent::decode(encoded).map_err(|_| PreparationError::KernelRejected)?;
            let admitted_pitch_millihertz =
                apply_event(execution, base, event).inspect_err(|_| {
                    let _ = base.quiesce();
                    execution.voices.fill(None);
                })?;
            if execution.normalized_events.len() == execution.normalized_events.capacity() {
                return Err(PreparationError::KernelRejected);
            }
            execution
                .normalized_events
                .push(NormalizedNoteEvidence::admitted(
                    event,
                    admitted_pitch_millihertz,
                ));
            event_writes = event_writes
                .checked_add(match event.gate {
                    Gate::On => 2,
                    Gate::Off => 1,
                })
                .ok_or(PreparationError::KernelRejected)?;
            events = events
                .checked_add(1)
                .ok_or(PreparationError::KernelRejected)?;
            complete(&mut execution.scheduler, request)?;
        }
        if matches!(status, SchedulerStatus::Drained) {
            let quiesce_writes = base
                .quiesce()
                .map_err(|_| PreparationError::KernelRejected)?;
            execution.voices.fill(None);
            return Ok(Opl2PlayReport {
                events,
                peak_voices: execution.peak_voices,
                reset_writes,
                patch_writes,
                event_writes,
                quiesce_writes,
                kernel_decisions: execution.scheduler.decisions(),
                kernel_signs: execution.scheduler.signs().len(),
                final_active_voices: 0,
                completed: true,
            });
        }
    }
    let _ = base.quiesce();
    execution.voices.fill(None);
    Err(PreparationError::KernelRejected)
}

pub fn cancel<B: Opl2Base>(
    execution: &mut PreparedOpl2Execution,
    base: &mut B,
) -> Result<u16, PreparationError> {
    execution
        .scheduler
        .cancel()
        .map_err(|_| PreparationError::KernelRejected)?;
    let writes = base
        .quiesce()
        .map_err(|_| PreparationError::KernelRejected)?;
    execution.voices.fill(None);
    Ok(writes)
}

mod evidence;
pub use evidence::{Opl2ConformanceReport, cancel_with_evidence, run_with_evidence};

mod voice;
use voice::apply_event;

fn complete(scheduler: &mut Scheduler, request: HostCallRequest) -> Result<(), PreparationError> {
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|_| PreparationError::KernelRejected)
}

fn validate_prepared(prepared: &PreparedOpl2Play) -> Result<(), PreparationError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(PreparationError::PlanRejected)?;
    if fragment.placements.len() != 3
        || fragment.connections.len() != 2
        || !fragment.placements.iter().any(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::MUSIC_PLAY_KIND
                && placement.implementation_id.as_str() == crate::opl2_offer::OPL2_IMPLEMENTATION
        })
        || prepared.active_play.plan_id != prepared.plan.plan_id
        || prepared.active_play.host_id != fragment.host_id
        || prepared.active_play.boot_id != fragment.boot_id
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

mod fixture;
pub use fixture::reviewed_values;

#[cfg(test)]
use fixture::note;

#[cfg(test)]
#[path = "opl2_play_tests.rs"]
mod tests;
