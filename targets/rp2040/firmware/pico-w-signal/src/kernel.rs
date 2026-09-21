//! Conduit kernel execution for the generated Signal demo image on Pico W.
//!
//! The build script parses `proof/fixtures/forms/signal-demo.conduit`, plans it onto the
//! Pico-local advertisement, lowers the exact fragment, and emits the fixed
//! tables consumed here.

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use conduit_kernel::{
    scheduler::{
        FixedScheduler, SchedulerStatus, StepInputBytes, StepIo, StepBack, StepOutcome,
    },
    BoundedValueRef, Failure, FailureCode, FixedSignLog, FixedValueStore, HostCallDisposition,
    HostCallId, HostCallOutcome, NodeId, PortId, RequestId, SignSink, ValueRef, ValueStorage,
};
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use conduit_signal::{
    decode_signal_bytes, encode_signal_fixed, signal_level_for_sequence, Signal,
    SIGNAL_ENCODED_LEN, SIGNAL_ENCODED_LEN_USIZE,
};
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use cyw43::Control;
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use embassy_time::{Duration, Timer};

#[cfg(not(feature = "wifi-bootstrap"))]
use crate::receipts::BootIdentity;
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use crate::receipts::PresentationReceiptIdentity;
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::receipts::TerminalIdentity;
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::signal_image::BOOT_SIGN_ID;
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
use crate::signal_image::{
    decode_wait_ms, generated_cords, generated_host_bindings, generated_nodes, generated_routes,
    presentation_identity, signal_layout, value_store_bytes, CORDS, EMPTY_VALUE_REF,
    HOST_BINDING_SLOTS, MAX_STORED_SIGNAL_VALUES, NODES, PENDING_REQUESTS, PORTS, QUEUE_SLOTS,
    ROUTE_SLOTS, ROUTE_TARGETS, RUNTIME_SIGN_BYTES, RUNTIME_SIGN_EVENTS, VALUE_SLOTS,
    WAIT_VALUE_BYTES,
};
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::signal_image::{
    ACTIVE_PLAY_ID, BOOT_ID, CHECKED_FORM_ID, EXPANDED_FORM_ID, FIRMWARE_BUILD_ID, FRAGMENT_ID,
    HOST_ID, PLAN_ID, SOURCE_DOCUMENT_ID, TERMINAL_SIGN_ID,
};

/// Run the generated local Signal demo through conduit-kernel.
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
pub async fn run_signal_demo(
    control: &mut Control<'_>,
    cdc: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) {
    let layout = signal_layout().expect("generated Signal image layout is valid");
    if cdc
        .write_boot_identity(boot_identity(), runtime)
        .await
        .is_err()
    {
        return;
    }
    let mut values = FixedValueStore::<VALUE_SLOTS, SIGNAL_ENCODED_LEN_USIZE>::new(
        value_store_bytes(layout.configuration.count),
    )
    .expect("value store capacity valid");

    let mut signal_values = [EMPTY_VALUE_REF; MAX_STORED_SIGNAL_VALUES];
    for (sequence, slot) in signal_values
        .iter_mut()
        .enumerate()
        .take(layout.configuration.count)
    {
        let signal = Signal {
            sequence: sequence as u64,
            level: signal_level_for_sequence(sequence as u64, layout.configuration.initial_level),
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

    let sign =
        FixedSignLog::<RUNTIME_SIGN_EVENTS>::new(RUNTIME_SIGN_BYTES).expect("sign log valid");
    let routes = generated_routes();
    let host_bindings = generated_host_bindings();

    let pulse = PulseBack::new(
        signal_values,
        wait_values,
        layout.configuration.count,
        layout.pulse_output_port,
        layout.wait_host_call,
    );
    let show = ShowBack::new(layout.show_input_port, layout.present_host_call);
    let backs = generated_backs(layout.pulse_node, layout.show_node, pulse, show);

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
    >::new_with_host_calls(
        generated_nodes(),
        generated_cords(),
        routes,
        host_bindings,
        backs,
        values,
        sign,
    )
    .expect("generated signal demo plan valid");

    let mut error = false;
    loop {
        match scheduler.step() {
            Ok(SchedulerStatus::Drained) => break,
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
                if req.node == layout.pulse_node && req.call == layout.wait_host_call {
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
                } else if req.node == layout.show_node && req.call == layout.present_host_call
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
                    if cdc
                        .write_receipt(
                            signal.sequence,
                            signal.level,
                            presentation_receipt_identity(identity),
                            runtime,
                        )
                        .await
                        .is_err()
                    {
                        fail_host_request(&mut scheduler, req.node, req.request);
                        error = true;
                        break;
                    }
                    complete_host_request(&mut scheduler, req.node, req.request);
                } else {
                    fail_host_request(&mut scheduler, req.node, req.request);
                    error = true;
                    break;
                }
            }
            Err(err) => {
                let _ = cdc.write_error(err, terminal_identity(), runtime).await;
                error = true;
                break;
            }
        }
    }

    let _ = cdc
        .write_terminal(!error, terminal_identity(), runtime)
        .await;
}

#[cfg(not(feature = "wifi-bootstrap"))]
pub fn boot_identity() -> BootIdentity {
    BootIdentity {
        firmware_build_id: FIRMWARE_BUILD_ID,
        source_document_id: SOURCE_DOCUMENT_ID,
        checked_form_id: CHECKED_FORM_ID,
        expanded_form_id: EXPANDED_FORM_ID,
        plan_id: PLAN_ID,
        fragment_id: FRAGMENT_ID,
        host_id: HOST_ID,
        boot_id: BOOT_ID,
        boot_sign_id: BOOT_SIGN_ID,
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
pub fn presentation_receipt_identity(
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
        sign_id: identity.sign_id,
    }
}

#[cfg(not(feature = "wifi-bootstrap"))]
pub fn terminal_identity() -> TerminalIdentity {
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
        sign_id: TERMINAL_SIGN_ID,
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
type SignalScheduler<S, E> = FixedScheduler<
    SignalBack,
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

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
fn complete_host_request<S: ValueStorage, E: SignSink>(
    scheduler: &mut SignalScheduler<S, E>,
    node: NodeId,
    request: RequestId,
) {
    scheduler
        .complete_host_call(
            node,
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .expect("host completion accepted");
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
fn fail_host_request<S: ValueStorage, E: SignSink>(
    scheduler: &mut SignalScheduler<S, E>,
    node: NodeId,
    request: RequestId,
) {
    let _ = scheduler.complete_host_call(
        node,
        request,
        HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: None,
        },
    );
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
fn generated_backs(
    pulse_node: NodeId,
    show_node: NodeId,
    pulse: PulseBack,
    show: ShowBack,
) -> [SignalBack; NODES] {
    match (pulse_node.0, show_node.0) {
        (0, 1) => [SignalBack::Pulse(pulse), SignalBack::Show(show)],
        (1, 0) => [SignalBack::Show(show), SignalBack::Pulse(pulse)],
        _ => panic!("generated Signal image must have one pulse and one show node"),
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "allocator-free firmware keeps Backs inline"
)]
#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
enum SignalBack {
    Pulse(PulseBack),
    Show(ShowBack),
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
impl StepBack<PORTS> for SignalBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Pulse(driver) => driver.step(io, input_bytes),
            Self::Show(driver) => driver.step(io, input_bytes),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Pulse(driver) => driver.cancel(),
            Self::Show(driver) => driver.cancel(),
        }
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
struct PulseBack {
    signal_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
    wait_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
    count: usize,
    output_port: PortId,
    wait_host_call: HostCallId,
    next: usize,
    pending_request: Option<RequestId>,
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
impl PulseBack {
    fn new(
        signal_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
        wait_values: [ValueRef; MAX_STORED_SIGNAL_VALUES],
        count: usize,
        output_port: PortId,
        wait_host_call: HostCallId,
    ) -> Self {
        Self {
            signal_values,
            wait_values,
            count,
            output_port,
            wait_host_call,
            next: 0,
            pending_request: None,
        }
    }

    fn fail(code: FailureCode, detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure { code, detail })
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
impl StepBack<PORTS> for PulseBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(expected) = self.pending_request {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected || io.consume_host_completion().is_err() {
                return Self::fail(FailureCode::InvalidLifecycle, 1);
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return Self::fail(FailureCode::HostCallFailed, 2);
            }
            self.pending_request = None;
        }

        if self.next >= self.count {
            return StepOutcome::Complete;
        }
        if !io.output_ready(self.output_port) {
            return StepOutcome::Await;
        }
        if io
            .send(self.output_port, self.signal_values[self.next])
            .is_err()
        {
            return Self::fail(FailureCode::InvalidLifecycle, 3);
        }
        self.next += 1;
        if self.next >= self.count {
            return StepOutcome::Complete;
        }
        let request = RequestId(self.next as u32);
        if io
            .request_host_call(
                request,
                self.wait_host_call,
                BoundedValueRef {
                    value: self.wait_values[self.next],
                    admitted_bytes: WAIT_VALUE_BYTES,
                },
            )
            .is_err()
        {
            return Self::fail(FailureCode::InvalidLifecycle, 4);
        }
        self.pending_request = Some(request);
        StepOutcome::Progress
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
struct ShowBack {
    input_port: PortId,
    present_host_call: HostCallId,
    pending_request: Option<RequestId>,
    presented: usize,
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
impl ShowBack {
    fn new(input_port: PortId, present_host_call: HostCallId) -> Self {
        Self {
            input_port,
            present_host_call,
            pending_request: None,
            presented: 0,
        }
    }

    fn fail(code: FailureCode, detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure { code, detail })
    }
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
impl StepBack<PORTS> for ShowBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(expected) = self.pending_request {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected || io.consume_host_completion().is_err() {
                return Self::fail(FailureCode::InvalidLifecycle, 10);
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return Self::fail(FailureCode::HostCallFailed, 11);
            }
            self.pending_request = None;
            self.presented += 1;
            return StepOutcome::Progress;
        }

        if let Some(value) = io.input(self.input_port) {
            let request = RequestId(self.presented as u32);
            if io.consume(self.input_port).is_err()
                || io
                    .request_host_call(
                        request,
                        self.present_host_call,
                        BoundedValueRef {
                            value,
                            admitted_bytes: SIGNAL_ENCODED_LEN,
                        },
                    )
                    .is_err()
            {
                return Self::fail(FailureCode::InvalidLifecycle, 9);
            }
            self.pending_request = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(self.input_port) {
            if io.consume_closed(self.input_port).is_err() {
                return Self::fail(FailureCode::InvalidLifecycle, 12);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
