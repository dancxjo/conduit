//! Conduit kernel execution for the generated Signal demo image on Pico W.
//!
//! The build script parses `examples/signal-demo.form`, plans it onto the
//! Pico-local advertisement, lowers the exact fragment, and emits the fixed
//! tables consumed here.

use conduit_kernel::{
    scheduler::{
        FixedScheduler, OperationDriver, SchedulerStatus, StepIo, StepOperation, StepOutcome,
    },
    BoundedValueRef, EvidenceSink, Failure, FailureCode, FixedEvidenceLog, FixedValueStore,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, NodeId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_signal::{
    decode_signal_bytes, encode_signal_fixed, signal_level_for_sequence, Signal,
    SIGNAL_ENCODED_LEN, SIGNAL_ENCODED_LEN_USIZE,
};
use cyw43::Control;
use embassy_time::{Duration, Timer};

use crate::receipts::{
    BootIdentity, PresentationReceiptIdentity, RuntimeTranscriptIdentity, TerminalIdentity, UsbCdc,
};
use crate::signal_image::{
    decode_wait_ms, generated_cords, generated_host_bindings, generated_nodes, generated_routes,
    presentation_identity, signal_layout, value_store_bytes, ACTIVE_PLAY_ID, BOOT_EVIDENCE_ID,
    BOOT_ID, CHECKED_FORM_ID, EMPTY_VALUE_REF, EXPANDED_FORM_ID, FIRMWARE_BUILD_ID, FRAGMENT_ID,
    HOST_BINDING_SLOTS, HOST_ID, MAX_STORED_SIGNAL_VALUES, NODES, PENDING_REQUESTS, PLAN_ID, PORTS,
    QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS, RUNTIME_EVIDENCE_BYTES, RUNTIME_EVIDENCE_EVENTS,
    SOURCE_DOCUMENT_ID, TERMINAL_EVIDENCE_ID, VALUE_SLOTS, WAIT_VALUE_BYTES, CORDS,
};

/// Run the generated local Signal demo through conduit-kernel.
pub async fn run_signal_demo(
    control: &mut Control<'_>,
    cdc: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) {
    let layout = signal_layout().expect("generated Signal image layout is valid");
    cdc.write_boot_identity(boot_identity(), runtime).await;
    let mut values =
        FixedValueStore::<VALUE_SLOTS, SIGNAL_ENCODED_LEN_USIZE>::new(value_store_bytes(
            layout.configuration.count,
        ))
        .expect("value store capacity valid");

    let mut signal_values = [EMPTY_VALUE_REF; MAX_STORED_SIGNAL_VALUES];
    for (sequence, slot) in signal_values
        .iter_mut()
        .enumerate()
        .take(layout.configuration.count)
    {
        let signal = Signal {
            sequence: sequence as u64,
            level: signal_level_for_sequence(
                sequence as u64,
                layout.configuration.initial_level,
            ),
        };
        *slot = values
            .store(&encode_signal_fixed(&signal))
            .expect("signal fits in generated store");
    }

    let mut wait_values = [EMPTY_VALUE_REF; MAX_STORED_SIGNAL_VALUES];
    let wait_bytes = layout.configuration.period_ms.to_le_bytes();
    for slot in wait_values
        .iter_mut()
        .take(layout.configuration.count)
        .skip(1)
    {
        *slot = values
            .store(&wait_bytes)
            .expect("wait duration fits in generated store");
    }

    let evidence =
        FixedEvidenceLog::<RUNTIME_EVIDENCE_EVENTS>::new(RUNTIME_EVIDENCE_BYTES)
            .expect("evidence log valid");
    let routes = generated_routes();
    let host_bindings = generated_host_bindings();

    let pulse = PulseDriver::new(
        signal_values,
        wait_values,
        layout.configuration.count,
        layout.pulse_output_port,
        layout.wait_operation,
    );
    let show = ShowDriver::new(layout.show_input_port, layout.present_operation);
    let drivers = generated_drivers(layout.pulse_node, layout.show_node, pulse, show);

    let mut scheduler = FixedScheduler::<
        _,
        _,
        _,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >::new_with_host_operations(
        generated_nodes(),
        generated_cords(),
        routes,
        host_bindings,
        drivers,
        values,
        evidence,
    )
    .expect("generated signal demo plan valid");

    let mut error = false;
    loop {
        match scheduler.step() {
            Ok(SchedulerStatus::Complete) => break,
            Ok(SchedulerStatus::Cancelled) => {
                error = true;
                break;
            }
            Ok(SchedulerStatus::Progress { .. }) => continue,
            Ok(SchedulerStatus::Idle) => {
                let Some(req) = scheduler.next_host_request() else {
                    error = true;
                    break;
                };
                if req.node == layout.pulse_node && req.operation == layout.wait_operation {
                    let duration_ms = scheduler
                        .host_value(req.input.value)
                        .ok()
                        .and_then(decode_wait_ms);
                    let Some(duration_ms) = duration_ms else {
                        fail_host_request(&mut scheduler, req.node, req.request);
                        error = true;
                        break;
                    };
                    Timer::after(Duration::from_millis(duration_ms)).await;
                    complete_host_request(&mut scheduler, req.node, req.request);
                } else if req.node == layout.show_node
                    && req.operation == layout.present_operation
                {
                    let signal = scheduler
                        .host_value(req.input.value)
                        .ok()
                        .and_then(|bytes| decode_signal_bytes(bytes).ok());
                    let Some(signal) = signal else {
                        fail_host_request(&mut scheduler, req.node, req.request);
                        error = true;
                        break;
                    };
                    let Some(identity) = presentation_identity(signal.sequence as usize) else {
                        fail_host_request(&mut scheduler, req.node, req.request);
                        error = true;
                        break;
                    };
                    control.gpio_set(0, signal.level).await;
                    cdc.write_receipt(
                        signal.sequence,
                        signal.level,
                        presentation_receipt_identity(identity),
                        runtime,
                    )
                    .await;
                    complete_host_request(&mut scheduler, req.node, req.request);
                } else {
                    fail_host_request(&mut scheduler, req.node, req.request);
                    error = true;
                    break;
                }
            }
            Err(err) => {
                cdc.write_error(err, terminal_identity(), runtime).await;
                error = true;
                break;
            }
        }
    }

    cdc.write_terminal(!error, terminal_identity(), runtime)
        .await;
}

fn boot_identity() -> BootIdentity {
    BootIdentity {
        firmware_build_id: FIRMWARE_BUILD_ID,
        source_document_id: SOURCE_DOCUMENT_ID,
        checked_form_id: CHECKED_FORM_ID,
        expanded_form_id: EXPANDED_FORM_ID,
        plan_id: PLAN_ID,
        fragment_id: FRAGMENT_ID,
        host_id: HOST_ID,
        boot_id: BOOT_ID,
        boot_evidence_id: BOOT_EVIDENCE_ID,
    }
}

fn presentation_receipt_identity(
    identity: crate::signal_image::PresentationIdentity,
) -> PresentationReceiptIdentity {
    PresentationReceiptIdentity {
        firmware_build_id: FIRMWARE_BUILD_ID,
        source_document_id: SOURCE_DOCUMENT_ID,
        checked_form_id: CHECKED_FORM_ID,
        expanded_form_id: EXPANDED_FORM_ID,
        plan_id: PLAN_ID,
        fragment_id: FRAGMENT_ID,
        host_id: HOST_ID,
        boot_id: BOOT_ID,
        active_play_id: ACTIVE_PLAY_ID,
        presentation_id: identity.presentation_id,
        evidence_id: identity.evidence_id,
    }
}

fn terminal_identity() -> TerminalIdentity {
    TerminalIdentity {
        firmware_build_id: FIRMWARE_BUILD_ID,
        source_document_id: SOURCE_DOCUMENT_ID,
        checked_form_id: CHECKED_FORM_ID,
        expanded_form_id: EXPANDED_FORM_ID,
        plan_id: PLAN_ID,
        fragment_id: FRAGMENT_ID,
        host_id: HOST_ID,
        boot_id: BOOT_ID,
        active_play_id: ACTIVE_PLAY_ID,
        evidence_id: TERMINAL_EVIDENCE_ID,
    }
}

type SignalScheduler<S, E> = FixedScheduler<
    SignalDriver,
    S,
    E,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

fn complete_host_request<S: ValueStorage, E: EvidenceSink>(
    scheduler: &mut SignalScheduler<S, E>,
    node: NodeId,
    request: RequestId,
) {
    scheduler
        .complete_host_operation(
            node,
            request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .expect("host completion accepted");
}

fn fail_host_request<S: ValueStorage, E: EvidenceSink>(
    scheduler: &mut SignalScheduler<S, E>,
    node: NodeId,
    request: RequestId,
) {
    let _ = scheduler.complete_host_operation(
        node,
        request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Failed,
            output: None,
            failure: None,
        },
    );
}

fn generated_drivers(
    pulse_node: NodeId,
    show_node: NodeId,
    pulse: PulseDriver,
    show: ShowDriver,
) -> [SignalDriver; NODES] {
    let pulse = OperationDriver::<_, PORTS>::new(pulse).expect("pulse driver valid");
    let show = OperationDriver::<_, PORTS>::new(show).expect("show driver valid");
    match (pulse_node.0, show_node.0) {
        (0, 1) => [SignalDriver::Pulse(pulse), SignalDriver::Show(show)],
        (1, 0) => [SignalDriver::Show(show), SignalDriver::Pulse(pulse)],
        _ => panic!("generated Signal image must have one pulse and one show node"),
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "allocator-free firmware keeps operation drivers inline"
)]
enum SignalDriver {
    Pulse(OperationDriver<PulseDriver, PORTS>),
    Show(OperationDriver<ShowDriver, PORTS>),
}

impl StepOperation<PORTS> for SignalDriver {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
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

struct PulseDriver {
    signal_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
    wait_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
    count: usize,
    output_port: PortId,
    wait_operation: HostOperationId,
    next: usize,
    pending_request: Option<RequestId>,
}

impl PulseDriver {
    fn new(
        signal_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
        wait_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
        count: usize,
        output_port: PortId,
        wait_operation: HostOperationId,
    ) -> Self {
        Self {
            signal_values,
            wait_values,
            count,
            output_port,
            wait_operation,
            next: 0,
            pending_request: None,
        }
    }

    fn emit_current(&self) -> OperationAction {
        if self.next >= self.count {
            return OperationAction::Complete;
        }
        OperationAction::Emit {
            port: self.output_port,
            value: self.signal_values[self.next],
        }
    }
}

impl Operation for PulseDriver {
    fn start(&mut self) -> OperationAction {
        self.emit_current()
    }

    fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.count {
            return OperationAction::Complete;
        }
        let req = RequestId(self.next as u32);
        self.pending_request = Some(req);
        OperationAction::RequestHostOperation {
            request: req,
            operation: self.wait_operation,
            input: BoundedValueRef {
                value: self.wait_values[self.next],
                admitted_bytes: WAIT_VALUE_BYTES,
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

struct ShowDriver {
    input_port: PortId,
    present_operation: HostOperationId,
    pending_request: Option<RequestId>,
    presented: usize,
}

impl ShowDriver {
    fn new(input_port: PortId, present_operation: HostOperationId) -> Self {
        Self {
            input_port,
            present_operation,
            pending_request: None,
            presented: 0,
        }
    }
}

impl Operation for ShowDriver {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value } => {
                if port != self.input_port {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 9,
                    });
                }
                let req = RequestId(self.presented as u32);
                self.pending_request = Some(req);
                OperationAction::RequestHostOperation {
                    request: req,
                    operation: self.present_operation,
                    input: BoundedValueRef {
                        value,
                        admitted_bytes: SIGNAL_ENCODED_LEN,
                    },
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
