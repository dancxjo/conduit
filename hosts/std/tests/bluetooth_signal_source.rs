#![cfg(feature = "bluetooth-bluez")]

use conduit_core::BaseImplementationId;
use conduit_signal_conformance::{exact_std_pico_bluetooth_plan, STD_PICO_USB_SINK_HOST_ID};
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_wire::{SessionMachine, SessionMessage, SessionRole, SessionTerminalDisposition};

#[test]
fn ordinary_kernel_signal_traffic_uses_the_exact_bluetooth_session() {
    let exact = exact_std_pico_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let source_host = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() != STD_PICO_USB_SINK_HOST_ID)
        .unwrap()
        .host_id
        .clone();
    let mut source = PicoUsbSource::prepare_plan(exact.plan, &source_host).unwrap();
    let binding = source.binding().clone();
    assert_eq!(
        binding.attachment.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();

    let hello = binding.hello_frame();
    source.admit_outbound(hello).unwrap();
    sink.admit_inbound(hello).unwrap();
    sink.admit_outbound(hello).unwrap();
    source.admit_inbound(hello).unwrap();
    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready).unwrap();
    sink.admit_inbound(ready).unwrap();
    sink.admit_outbound(ready).unwrap();
    source.admit_inbound(ready).unwrap();

    let mut delivered = 0_u64;
    while let Some((sequence, payload)) = source.next_offer().unwrap() {
        let offered = binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload,
        });
        source.admit_outbound(offered).unwrap();
        sink.admit_inbound(offered).unwrap();
        let accepted = binding.frame(SessionMessage::Accepted { sequence });
        sink.admit_outbound(accepted).unwrap();
        source.admit_inbound(accepted).unwrap();
        source.accepted(sequence).unwrap();
        let delivered_fact = binding.frame(SessionMessage::Delivered { sequence });
        sink.admit_outbound(delivered_fact).unwrap();
        source.admit_inbound(delivered_fact).unwrap();
        source.delivered(sequence).unwrap();
        delivered += 1;
    }
    assert_eq!(source.finish_kernel().unwrap(), delivered);
    assert_eq!(delivered, 16);

    let closed = binding.frame(SessionMessage::InputClosed {
        final_sequence: delivered,
    });
    source.admit_outbound(closed).unwrap();
    sink.admit_inbound(closed).unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence: delivered,
    });
    source.admit_outbound(terminal).unwrap();
    sink.admit_inbound(terminal).unwrap();
    sink.admit_outbound(terminal).unwrap();
    source.admit_inbound(terminal).unwrap();
    assert!(source.is_terminal());
    assert!(sink.is_terminal());
}
