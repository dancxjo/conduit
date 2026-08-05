//! Conduit kernel execution for the Signal demo on Pico W.
//!
//! Pre-encodes all 16 signal values into a fixed-size FixedScheduler plan,
//! drives Embassy timers for waits, and calls the radio and receipt modules.
//! No heap allocator is used; all storage is statically sized.

use conduit_kernel::{
    Failure, FailureCode,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, SchedulerStatus,
        StepIo, StepOperation, StepOutcome,
    },
    BoundedValueRef, CordEndpoint, CordId, FixedEvidenceLog, FixedHostOperationBindings,
    FixedRoutes, FixedValueStore, HostOperationBinding, HostOperationDisposition,
    HostOperationId, HostOperationOutcome, NodeId, Operation, OperationAction, OperationInput,
    PortId, RequestId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use cyw43::Control;
use embassy_time::{Duration, Timer};

use crate::receipts::UsbCdc;

// Signal encoding constants (matches conduit-signal: 8-byte LE sequence + 1-byte level).
const SIGNAL_ENCODED_LEN: u32 = 9;

/// Decode 9 encoded signal bytes into (sequence, level).
fn decode_signal_bytes(encoded: &[u8]) -> Option<(u64, bool)> {
    if encoded.len() != SIGNAL_ENCODED_LEN as usize {
        return None;
    }
    let mut seq_bytes = [0u8; 8];
    seq_bytes.copy_from_slice(&encoded[..8]);
    Some((u64::from_le_bytes(seq_bytes), encoded[8] != 0))
}

// Signal demo fixed parameters (matches signal-demo.form)
const COUNT: usize = 16;
const PERIOD_MS: u64 = 250;
const INITIAL_LEVEL: bool = false;

// Plan topology constants
const NODE_PULSE: NodeId = NodeId(0);
const NODE_SHOW: NodeId = NodeId(1);
const CORD_SIGNAL: CordId = CordId(0);
const PORT_SIGNAL: PortId = PortId(0);

// Host operation IDs
const HOP_WAIT: HostOperationId = HostOperationId(0);
const HOP_PRESENT: HostOperationId = HostOperationId(1);

/// Encode a signal value into 9 bytes: 8 bytes sequence LE + 1 byte level.
fn encode_signal_bytes(seq: u64, level: bool) -> [u8; 9] {
    let mut buf = [0u8; 9];
    buf[..8].copy_from_slice(&seq.to_le_bytes());
    buf[8] = level as u8;
    buf
}

fn signal_level(seq: u64) -> bool {
    if seq % 2 == 0 { INITIAL_LEVEL } else { !INITIAL_LEVEL }
}

/// Run the fixed 16-signal local demo through conduit-kernel.
pub async fn run_signal_demo(control: &mut Control<'_>, cdc: &mut UsbCdc) {
    // Value store: 16 slots, each up to 9 bytes
    let mut values =
        FixedValueStore::<16, 9>::new(SIGNAL_ENCODED_LEN * COUNT as u32)
            .expect("value store capacity valid");

    // Pre-store all 16 encoded signal values
    let mut value_refs = [ValueRef { slot: 0, generation: 0, byte_len: 0 }; COUNT];
    for i in 0..COUNT {
        let bytes = encode_signal_bytes(i as u64, signal_level(i as u64));
        value_refs[i] = values.store(&bytes).expect("signal fits in store");
    }

    let evidence = FixedEvidenceLog::<64>::new(1024).expect("evidence log valid");

    let pulse = PulseDriver::new(value_refs);
    let show = ShowDriver::new();

    // Node specs (PORTS = 2 max)
    let node_specs = [
        NodeSpec { input_cords: [None, None], maximum_step_work: 8 },
        NodeSpec { input_cords: [Some(CORD_SIGNAL), None], maximum_step_work: 8 },
    ];

    let cord_specs = [CordSpec::local(
        CORD_SIGNAL,
        (NODE_PULSE, PORT_SIGNAL),
        (NODE_SHOW, PORT_SIGNAL),
        CordCapacity { slot_start: 0, item_capacity: 4, byte_capacity: SIGNAL_ENCODED_LEN * 4 },
    )];

    // Routes: ROUTE_SLOTS = max port index across nodes, TARGETS = number of route targets
    // Pulse node 0, port 0 -> cord 0 sink (Show node 1, port 0)
    let mut routes = FixedRoutes::<2, 1>::new(2);
    routes.install(
        NODE_PULSE,
        PORT_SIGNAL,
        RouteRange { start: 0, len: 1 },
        &[RouteTarget {
            cord: CORD_SIGNAL,
            sink: CordEndpoint::local(NODE_SHOW, PORT_SIGNAL),
        }],
    ).expect("route table valid");
    routes.seal().expect("route table sealed");

    // Host operation bindings: per-node
    // Pulse (node 0): wait (HOP_WAIT), no input bytes (timer has no input), no output bytes
    // Show (node 1): present (HOP_PRESENT), input = signal bytes
    let mut host_bindings = FixedHostOperationBindings::<4>::new(2);
    host_bindings.install(
        NODE_PULSE,
        HostOperationBinding {
            operation: HOP_WAIT,
            maximum_input_bytes: SIGNAL_ENCODED_LEN, // must be > 0 per API check
            maximum_output_bytes: 0,
        },
    ).expect("wait binding valid");
    host_bindings.install(
        NODE_SHOW,
        HostOperationBinding {
            operation: HOP_PRESENT,
            maximum_input_bytes: SIGNAL_ENCODED_LEN,
            maximum_output_bytes: 0,
        },
    ).expect("present binding valid");
    host_bindings.seal().expect("host bindings sealed");

    let pulse_driver = OperationDriver::<_, 2>::new(pulse).expect("pulse driver valid");
    let show_driver = OperationDriver::<_, 2>::new(show).expect("show driver valid");
    let drivers = [
        SignalDriver::Pulse(pulse_driver),
        SignalDriver::Show(show_driver),
    ];

    // FixedScheduler type params:
    // D, S, E, NODES=2, CORDS=1, PORTS=2, QUEUE_SLOTS=4,
    // ROUTE_SLOTS=2, ROUTE_TARGETS=1, HOST_BINDING_SLOTS=4, PENDING_REQUESTS=2
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 4, 2, 1, 4, 2>::new_with_host_operations(
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        evidence,
    ).expect("signal demo plan valid");

    let mut error = false;
    loop {
        match scheduler.step() {
            Ok(SchedulerStatus::Complete) => break,
            Ok(SchedulerStatus::Cancelled) => { error = true; break; }
            Ok(SchedulerStatus::Progress { .. }) => continue,
            Ok(SchedulerStatus::Idle) => {
                let Some(req) = scheduler.next_host_request() else {
                    error = true; // idle with no pending request = deadlock
                    break;
                };
                match req.operation {
                    HOP_WAIT => {
                        Timer::after(Duration::from_millis(PERIOD_MS)).await;
                        scheduler.complete_host_operation(
                            req.node,
                            req.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        ).expect("wait completion accepted");
                    }
                    HOP_PRESENT => {
                        let bytes = scheduler.host_value(req.input.value)
                            .expect("present value in store");
                        let (sequence, level) = decode_signal_bytes(bytes)
                            .expect("valid signal encoding");
                        // Drive CYW43 onboard LED (GPIO 0)
                        control.gpio_set(0, level).await;
                        // Emit machine-readable USB receipt
                        cdc.write_receipt(sequence, level).await;
                        scheduler.complete_host_operation(
                            req.node,
                            req.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        ).expect("present completion accepted");
                    }
                    _ => {
                        let _ = scheduler.complete_host_operation(
                            req.node,
                            req.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Failed,
                                output: None,
                                failure: None,
                            },
                        );
                        error = true;
                        break;
                    }
                }
            }
            Err(err) => { cdc.write_error(err).await; error = true; break; }
        }
    }

    cdc.write_terminal(!error).await;
}

enum SignalDriver {
    Pulse(OperationDriver<PulseDriver, 2>),
    Show(OperationDriver<ShowDriver, 2>),
}

impl StepOperation<2> for SignalDriver {
    fn step(&mut self, io: &mut StepIo<2>) -> StepOutcome {
        match self {
            Self::Pulse(driver) => driver.step(io),
            Self::Show(driver) => driver.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Pulse(driver) => driver.cancel(),
            Self::Show(driver) => driver.cancel(),
        }
    }
}

/// Pulse operation: emits pre-stored signal values, requests timer waits between emissions.
struct PulseDriver {
    values: [ValueRef; COUNT],
    next: usize,
    state: PulseState,
    pending_request: Option<RequestId>,
}

#[derive(Clone, Copy, PartialEq)]
enum PulseState { Ready, AwaitingTimer }

impl PulseDriver {
    fn new(values: [ValueRef; COUNT]) -> Self {
        Self { values, next: 0, state: PulseState::Ready, pending_request: None }
    }
}

impl Operation for PulseDriver {
    fn start(&mut self) -> OperationAction {
        self.emit_current()
    }

    fn advance(&mut self) -> OperationAction {
        // Called after Emit is accepted by the scheduler
        self.next += 1;
        if self.next >= COUNT {
            return OperationAction::Complete;
        }
        let req = RequestId(self.next as u32);
        self.pending_request = Some(req);
        self.state = PulseState::AwaitingTimer;
        // For the wait, we pass the next signal as the "input" value so the
        // host operation has a non-zero input (required by the binding check).
        OperationAction::RequestHostOperation {
            request: req,
            operation: HOP_WAIT,
            input: BoundedValueRef {
                value: self.values[self.next],
                admitted_bytes: SIGNAL_ENCODED_LEN,
            },
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome } => {
                if Some(request) != self.pending_request {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 1,
                    });
                }
                self.pending_request = None;
                self.state = PulseState::Ready;
                match outcome.disposition {
                    HostOperationDisposition::Completed => self.emit_current(),
                    _ => OperationAction::Fail(Failure {
                        code: FailureCode::HostOperationFailed,
                        detail: 2,
                    }),
                }
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 3,
            }),
        }
    }
}

impl PulseDriver {
    fn emit_current(&self) -> OperationAction {
        if self.next >= COUNT {
            return OperationAction::Complete;
        }
        OperationAction::Emit { port: PORT_SIGNAL, value: self.values[self.next] }
    }
}

/// Show operation: receives signal values and requests LED presentation.
struct ShowDriver {
    pending_request: Option<RequestId>,
    presented: usize,
}

impl ShowDriver {
    fn new() -> Self {
        Self { pending_request: None, presented: 0 }
    }
}

impl Operation for ShowDriver {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port: _, value } => {
                let req = RequestId(self.presented as u32);
                self.pending_request = Some(req);
                OperationAction::RequestHostOperation {
                    request: req,
                    operation: HOP_PRESENT,
                    input: BoundedValueRef { value, admitted_bytes: SIGNAL_ENCODED_LEN },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome } => {
                if Some(request) != self.pending_request {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 10,
                    });
                }
                self.pending_request = None;
                self.presented += 1;
                match outcome.disposition {
                    HostOperationDisposition::Completed => OperationAction::Await,
                    _ => OperationAction::Fail(Failure {
                        code: FailureCode::HostOperationFailed,
                        detail: 11,
                    }),
                }
            }
            OperationInput::Closed { .. } => OperationAction::Complete,
        }
    }
}
