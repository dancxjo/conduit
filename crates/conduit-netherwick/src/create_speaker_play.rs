//! Production-kernel lifecycle proof for one planned Create speaker song.

use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver,
        SchedulerStatus,
    },
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, KernelEvent, NodeId, Operation, OperationAction, OperationInput, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueRef, ValueStorage,
};

use crate::{
    EncodedSong, MAXIMUM_ADMITTED_SERIAL_BYTES, PLAY_SONG_OPCODE, SONG_OPCODE, SPEAKER_CAPABILITY,
    SPEAKER_IMPLEMENTATION, SPEAKER_OPERATION,
};

const SOURCE_NODE: NodeId = NodeId(0);
const SPEAKER_NODE: NodeId = NodeId(1);
const OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 1;
const SIGNS: usize = 64;

#[derive(Clone, Copy)]
struct Source {
    value: Option<ValueRef>,
}

impl Operation for Source {
    fn start(&mut self) -> OperationAction {
        self.value
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }

    fn advance(&mut self) -> OperationAction {
        self.value = None;
        OperationAction::Complete
    }

    fn resume(&mut self, _: OperationInput) -> OperationAction {
        invalid(1)
    }

    fn cancel(&mut self) {
        self.value = None;
    }
}

#[derive(Clone, Copy)]
struct SpeakerOperation {
    pending: bool,
}

impl Operation for SpeakerOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                self.pending = true;
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_ADMITTED_SERIAL_BYTES as u32)
                else {
                    return invalid(2);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(1),
                    operation: OPERATION,
                    input,
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(1),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(3),
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

#[derive(Clone, Copy)]
enum DriverOperation {
    Source(Source),
    Speaker(SpeakerOperation),
}

impl Operation for DriverOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.start(),
            Self::Speaker(value) => value.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(value) => value.resume(input),
            Self::Speaker(value) => value.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.advance(),
            Self::Speaker(value) => value.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.cancel(),
            Self::Speaker(value) => value.cancel(),
        }
    }
}

type Scheduler = FixedScheduler<
    OperationDriver<DriverOperation, PORTS>,
    FixedValueStore<2, MAXIMUM_ADMITTED_SERIAL_BYTES>,
    FixedSignLog<SIGNS>,
    2,
    1,
    PORTS,
    1,
    1,
    1,
    2,
    1,
>;

pub struct PreparedSpeakerExecution {
    scheduler: Scheduler,
    pending_request: Option<HostOperationRequest>,
    dispatched: bool,
    song_number: u8,
    define_bytes: u16,
    play_bytes: u16,
    maximum_completion_ticks: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerialFailure {
    ProviderLost,
    PartialWrite,
    DeviceNoResponse,
    TruncatedResponse,
    MalformedResponse,
    Refused,
    SongNotObserved,
}

pub trait CreateSpeakerSerial {
    fn write_exact(&mut self, bytes: &[u8]) -> Result<(), SerialFailure>;
    fn observe_song_playing(&mut self, song_number: u8) -> Result<bool, SerialFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeakerTerminal {
    Completed,
    CancelledBeforeDispatch,
    CancelledAfterDispatch { maximum_remaining_ticks: u16 },
    Failed(SerialFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeakerPlayReport {
    pub terminal: SpeakerTerminal,
    pub define_bytes: u16,
    pub play_bytes: u16,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
}

pub fn prepare_speaker_execution(
    plan: &conduit_core::Plan,
    encoded: &EncodedSong,
) -> Result<PreparedSpeakerExecution, &'static str> {
    validate_plan(plan)?;
    if encoded.define.first() != Some(&SONG_OPCODE)
        || encoded.play[0] != PLAY_SONG_OPCODE
        || encoded.define.get(1) != Some(&encoded.play[1])
        || encoded.admitted_serial_bytes != encoded.define.len() + encoded.play.len()
        || encoded.admitted_serial_bytes > MAXIMUM_ADMITTED_SERIAL_BYTES
    {
        return Err("encoded song does not match the planned Create speaker operation");
    }
    let mut command = [0_u8; MAXIMUM_ADMITTED_SERIAL_BYTES];
    let split = encoded.define.len();
    command[..split].copy_from_slice(&encoded.define);
    command[split..encoded.admitted_serial_bytes].copy_from_slice(&encoded.play);
    let mut values = FixedValueStore::<2, MAXIMUM_ADMITTED_SERIAL_BYTES>::new(
        MAXIMUM_ADMITTED_SERIAL_BYTES as u32,
    )
    .map_err(|_| "value admission failed")?;
    let value = values
        .store(&command[..encoded.admitted_serial_bytes])
        .map_err(|_| "song storage admission failed")?;
    let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
    routes
        .install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(SPEAKER_NODE, PortId(0)),
            }],
        )
        .map_err(|_| "route admission failed")?;
    routes.seal().map_err(|_| "route seal failed")?;
    let mut bindings = FixedHostOperationBindings::<2>::new(1);
    bindings
        .install(
            SPEAKER_NODE,
            HostOperationBinding {
                operation: OPERATION,
                maximum_input_bytes: MAXIMUM_ADMITTED_SERIAL_BYTES as u32,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "host operation admission failed")?;
    bindings.seal().map_err(|_| "host operation seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "sign admission failed")?;
    let scheduler = FixedScheduler::new_with_host_operations(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 2,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (SOURCE_NODE, PortId(0)),
            (SPEAKER_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: MAXIMUM_ADMITTED_SERIAL_BYTES as u32,
            },
        )],
        routes,
        bindings,
        [
            OperationDriver::new(DriverOperation::Source(Source { value: Some(value) }))
                .map_err(|_| "source preparation failed")?,
            OperationDriver::new(DriverOperation::Speaker(SpeakerOperation {
                pending: false,
            }))
            .map_err(|_| "speaker preparation failed")?,
        ],
        values,
        signs,
    )
    .map_err(|_| "kernel preparation failed")?;
    Ok(PreparedSpeakerExecution {
        scheduler,
        pending_request: None,
        dispatched: false,
        song_number: encoded.play[1],
        define_bytes: encoded.define.len() as u16,
        play_bytes: encoded.play.len() as u16,
        maximum_completion_ticks: encoded.maximum_completion_ticks,
    })
}

pub fn run_speaker_execution<S: CreateSpeakerSerial>(
    execution: &mut PreparedSpeakerExecution,
    serial: &mut S,
) -> SpeakerPlayReport {
    if let Err(report) = dispatch_speaker_execution(execution, serial) {
        return report;
    }
    finish_speaker_execution(execution, serial)
}

pub fn dispatch_speaker_execution<S: CreateSpeakerSerial>(
    execution: &mut PreparedSpeakerExecution,
    serial: &mut S,
) -> Result<(), SpeakerPlayReport> {
    for _ in 0..32 {
        if execution.scheduler.step().is_err() {
            return Err(report(
                execution,
                SpeakerTerminal::Failed(SerialFailure::Refused),
                0,
                0,
            ));
        }
        if let Some(request) = execution.scheduler.next_host_request() {
            let bytes = match execution.scheduler.host_value(request.input.value) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return Err(report(
                        execution,
                        SpeakerTerminal::Failed(SerialFailure::Refused),
                        0,
                        0,
                    ));
                }
            };
            let define_len = bytes.len().saturating_sub(2);
            if let Err(failure) = serial.write_exact(&bytes[..define_len]) {
                return Err(fail_request(execution, request, failure, 0, 0));
            }
            if let Err(failure) = serial.write_exact(&bytes[define_len..]) {
                return Err(fail_request(
                    execution,
                    request,
                    failure,
                    define_len as u16,
                    0,
                ));
            }
            execution.pending_request = Some(request);
            execution.dispatched = true;
            return Ok(());
        }
    }
    Err(report(
        execution,
        SpeakerTerminal::Failed(SerialFailure::Refused),
        0,
        0,
    ))
}

pub fn finish_speaker_execution<S: CreateSpeakerSerial>(
    execution: &mut PreparedSpeakerExecution,
    serial: &mut S,
) -> SpeakerPlayReport {
    let Some(request) = execution.pending_request.take() else {
        return report(
            execution,
            SpeakerTerminal::Failed(SerialFailure::Refused),
            0,
            0,
        );
    };
    match serial.observe_song_playing(execution.song_number) {
        Ok(true) => complete_request(execution, request),
        Ok(false) => {
            return fail_request(
                execution,
                request,
                SerialFailure::SongNotObserved,
                execution.define_bytes,
                execution.play_bytes,
            );
        }
        Err(failure) => {
            return fail_request(
                execution,
                request,
                failure,
                execution.define_bytes,
                execution.play_bytes,
            );
        }
    }
    for _ in 0..32 {
        match execution.scheduler.step() {
            Ok(SchedulerStatus::Complete) => {
                return report(
                    execution,
                    SpeakerTerminal::Completed,
                    execution.define_bytes,
                    execution.play_bytes,
                );
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    report(
        execution,
        SpeakerTerminal::Failed(SerialFailure::Refused),
        execution.define_bytes,
        execution.play_bytes,
    )
}

pub fn cancel_speaker_execution(execution: &mut PreparedSpeakerExecution) -> SpeakerPlayReport {
    let dispatched = execution.dispatched;
    let _ = execution.scheduler.cancel();
    let terminal = if dispatched {
        SpeakerTerminal::CancelledAfterDispatch {
            maximum_remaining_ticks: execution.maximum_completion_ticks,
        }
    } else {
        SpeakerTerminal::CancelledBeforeDispatch
    };
    report(execution, terminal, 0, 0)
}

fn validate_plan(plan: &conduit_core::Plan) -> Result<(), &'static str> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.capability_id.as_str() == SPEAKER_CAPABILITY)
        .ok_or("Plan has no Create speaker placement")?;
    if placement.implementation_id.as_str() != SPEAKER_IMPLEMENTATION
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != SPEAKER_OPERATION
        || placement.authority.len() != 1
    {
        return Err("Plan does not seal the exact Create speaker contract");
    }
    Ok(())
}

fn complete_request(execution: &mut PreparedSpeakerExecution, request: HostOperationRequest) {
    let _ = execution.scheduler.complete_host_operation(
        request.node,
        request.request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: None,
            failure: None,
        },
    );
}

fn fail_request(
    execution: &mut PreparedSpeakerExecution,
    request: HostOperationRequest,
    failure: SerialFailure,
    define_bytes: u16,
    play_bytes: u16,
) -> SpeakerPlayReport {
    let _ = execution.scheduler.complete_host_operation(
        request.node,
        request.request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Failed,
            output: None,
            failure: Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostOperationFailed,
                detail: failure as u16,
            }),
        },
    );
    report(
        execution,
        SpeakerTerminal::Failed(failure),
        define_bytes,
        play_bytes,
    )
}

fn report(
    execution: &PreparedSpeakerExecution,
    terminal: SpeakerTerminal,
    define_bytes: u16,
    play_bytes: u16,
) -> SpeakerPlayReport {
    SpeakerPlayReport {
        terminal,
        define_bytes,
        play_bytes,
        kernel_decisions: execution.scheduler.decisions(),
        kernel_signs: execution.scheduler.signs().len(),
    }
}

#[cfg(test)]
#[path = "create_speaker_play_tests.rs"]
mod tests;
