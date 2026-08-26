use conduit_core::{BootId, ConnectionBase};
use conduit_signal::SIGNAL_ENCODED_LEN;
use conduit_signal_conformance::exact_std_pico_usb_plan;
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_wire::{
    SessionFrame, SessionMachine, SessionMessage, SessionRole, SessionTerminalDisposition,
    WireError,
};

fn trigger(source: &mut PicoUsbSource, sink: &mut SessionMachine) {
    let binding = source.binding().clone();
    let hello = binding.hello_frame();
    source.admit_outbound(hello).expect("source hello");
    sink.admit_inbound(hello).expect("sink receives hello");
    sink.admit_outbound(hello).expect("sink hello");
    source.admit_inbound(hello).expect("source receives hello");
    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready).expect("source ready");
    sink.admit_inbound(ready).expect("sink receives ready");
    sink.admit_outbound(ready).expect("sink ready");
    source.admit_inbound(ready).expect("source receives ready");
    assert!(source.is_active());
    assert!(sink.is_active());
}

fn admit_reciprocal_failure(
    source: &mut PicoUsbSource,
    sink: &mut SessionMachine,
    message: SessionMessage<'_>,
    disposition: SessionTerminalDisposition,
) {
    let binding = source.binding().clone();
    let failure = binding.frame(message);
    source.admit_outbound(failure).expect("source failure fact");
    sink.admit_inbound(failure).expect("sink receives failure");
    sink.admit_outbound(failure).expect("sink failure fact");
    source
        .admit_inbound(failure)
        .expect("source receives failure");
    source.cancel().expect("source kernel cancellation");

    let terminal = binding.frame(SessionMessage::Terminal {
        disposition,
        final_sequence: 0,
    });
    source
        .admit_outbound(terminal)
        .expect("source terminal fact");
    sink.admit_inbound(terminal)
        .expect("sink receives terminal");
    sink.admit_outbound(terminal).expect("sink terminal fact");
    source
        .admit_inbound(terminal)
        .expect("source receives terminal");
    assert!(source.is_terminal());
    assert!(sink.is_terminal());
}

#[test]
fn source_is_the_exact_planned_kernel_egress() {
    let source = PicoUsbSource::prepare().expect("source prepares");
    let exact = exact_std_pico_usb_plan().expect("plan resolves");
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
        .expect("sink fragment");
    assert_eq!(source.fragment().plan_id, sink.plan_id);
    assert_eq!(source.binding().sink_fragment_id, sink.fragment_id);
    assert_eq!(source.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(source.binding().limits.maximum_in_flight_items, 1);
    assert_eq!(
        source.binding().limits.maximum_payload_bytes,
        SIGNAL_ENCODED_LEN
    );
}

#[test]
fn observed_boot_rebinding_rejects_stale_boot_and_base_instance() {
    let mut source = PicoUsbSource::prepare().expect("source prepares");
    let planned = source.binding().clone();
    source
        .observe_sink_boot(BootId::from("observed-pico-runtime-boot"))
        .expect("runtime boot rebinds");

    assert_eq!(
        source.admit_inbound(planned.hello_frame()),
        Err(format!("{:?}", WireError::BootMismatch))
    );

    let binding = source.binding().clone();
    let mut message = binding.hello_frame().message;
    if let conduit_wire::SessionMessage::Hello(ref mut hello) = message {
        hello.base_instance_id = "wrong-base-instance";
    }
    assert_eq!(
        source.admit_inbound(SessionFrame {
            identity: binding.identity(),
            message,
        }),
        Err(format!("{:?}", WireError::SessionEpochMismatch))
    );
}

#[test]
fn exact_source_and_sink_reach_two_sided_cancelled_terminal() {
    let mut source = PicoUsbSource::prepare().expect("source prepares");
    let mut sink =
        SessionMachine::new(source.binding().clone(), SessionRole::Sink).expect("sink session");
    trigger(&mut source, &mut sink);
    admit_reciprocal_failure(
        &mut source,
        &mut sink,
        SessionMessage::Cancelled { code: 7 },
        SessionTerminalDisposition::Cancelled,
    );
}

#[test]
fn exact_source_and_sink_reach_two_sided_failed_terminal() {
    let mut source = PicoUsbSource::prepare().expect("source prepares");
    let mut sink =
        SessionMachine::new(source.binding().clone(), SessionRole::Sink).expect("sink session");
    trigger(&mut source, &mut sink);
    admit_reciprocal_failure(
        &mut source,
        &mut sink,
        SessionMessage::Failed { code: 9 },
        SessionTerminalDisposition::Failed,
    );
}
