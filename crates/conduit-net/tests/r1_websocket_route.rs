use conduit_core::{BootId, ConnectionBase};
use conduit_net::{
    r1_route_basis, r1_websocket_probe_binding, R1_MAXIMUM_FRAME_BYTES, R1_PICO_HOST_ID,
    R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
};
use conduit_wire::{SessionMachine, SessionMessage, SessionRole};

#[test]
fn one_boot_has_two_exact_bounded_route_facts() {
    let boot = BootId::from("pico/runtime-boot");
    let [usb, websocket] = r1_route_basis(boot.clone());
    assert_eq!(usb.sink.host_id.as_str(), R1_PICO_HOST_ID);
    assert_eq!(websocket.sink.host_id, usb.sink.host_id);
    assert_eq!(websocket.sink.boot_id, boot);
    assert_eq!(websocket.sink.boot_id, usb.sink.boot_id);
    assert_eq!(usb.base, ConnectionBase::UsbCdc);
    assert_eq!(websocket.base, ConnectionBase::WebSocket);
    assert_ne!(usb.binding_id, websocket.binding_id);
    assert_ne!(usb.base_instance_id, websocket.base_instance_id);
    assert_ne!(usb.source.endpoint_id, websocket.source.endpoint_id);
    assert_ne!(usb.sink.endpoint_id, websocket.sink.endpoint_id);
    assert_eq!(websocket.limits.maximum_frame_bytes, R1_MAXIMUM_FRAME_BYTES);
    assert_eq!(
        websocket.limits.maximum_payload_bytes,
        R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES
    );
}

#[test]
fn probe_session_consumes_the_exact_websocket_route() {
    let binding = r1_websocket_probe_binding(BootId::from("pico/runtime-boot"));
    binding.validate().unwrap();
    assert_eq!(binding.attachment.base, ConnectionBase::WebSocket);
    assert_eq!(binding.sink.host_id.as_str(), R1_PICO_HOST_ID);
    assert_eq!(binding.sink.boot_id.as_str(), "pico/runtime-boot");

    let mut wrong = binding.clone();
    wrong.attachment.sink_boot_id = BootId::from("pico/stale-boot");
    assert!(wrong.validate().is_err());
}

#[test]
fn wrong_host_boot_instance_and_endpoint_each_fail_closed() {
    let binding = r1_websocket_probe_binding(BootId::from("pico/runtime-boot"));
    for mutation in 0..4 {
        let mut frame = binding.hello_frame();
        match mutation {
            0 => frame.identity.sink_host_id = "pico/wrong-host",
            1 => frame.identity.sink_boot_id = "pico/stale-boot",
            2 => match &mut frame.message {
                SessionMessage::Hello(hello) => {
                    hello.base_instance_id = "r1/wrong-websocket-instance"
                }
                _ => unreachable!(),
            },
            3 => match &mut frame.message {
                SessionMessage::Hello(hello) => hello.sink_endpoint_id = "r1/wrong-endpoint",
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        assert!(sink.admit_inbound(frame).is_err());
    }
}
