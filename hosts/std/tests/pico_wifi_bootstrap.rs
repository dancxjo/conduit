use conduit_core::{BootId, ConnectionBase};
use conduit_std_host::pico_wifi_bootstrap::PicoWifiBootstrapSource;
use conduit_wire::{SessionFrame, SessionMachine, SessionMessage, SessionRole, WireError};

fn activate(source: &mut PicoWifiBootstrapSource, sink: &mut SessionMachine) {
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

#[test]
fn source_emits_exact_bounded_runtime_info_without_putting_secrets_in_the_plan() {
    let ssid = b"ordinary-network";
    let credential = b"temporary-secret";
    let mut source = PicoWifiBootstrapSource::prepare(ssid, credential).expect("source prepares");
    let plan_json = serde_json::to_string(source.fragment()).expect("fragment serializes");
    assert!(!plan_json.contains("ordinary-network"));
    assert!(!plan_json.contains("temporary-secret"));
    assert_eq!(source.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(source.binding().limits.maximum_in_flight_items, 1);
    assert_eq!(
        source.binding().limits.maximum_payload_bytes,
        conduit_net::MAXIMUM_JOIN_INPUT_BYTES
    );

    let mut sink =
        SessionMachine::new(source.binding().clone(), SessionRole::Sink).expect("sink session");
    activate(&mut source, &mut sink);

    let (sequence, payload, payload_len) = source
        .next_offer()
        .expect("source advances")
        .expect("credential Info offered");
    assert_eq!(sequence, 0);
    let decoded = conduit_net::decode_network_join_request(&payload[..payload_len])
        .expect("credential Info decodes");
    assert_eq!(decoded.ssid, ssid);
    assert_eq!(decoded.credential, credential);

    let binding = source.binding().clone();
    let offered = binding.frame(SessionMessage::Offered {
        sequence,
        payload: &payload[..payload_len],
    });
    source.admit_outbound(offered).expect("source offers");
    sink.admit_inbound(offered).expect("sink receives offer");

    let accepted = binding.frame(SessionMessage::Accepted { sequence });
    sink.admit_outbound(accepted).expect("sink accepts");
    source.admit_inbound(accepted).expect("source sees accept");
    source.accepted(sequence).expect("kernel sees accept");

    let delivered = binding.frame(SessionMessage::Delivered { sequence });
    sink.admit_outbound(delivered).expect("sink delivers");
    source
        .admit_inbound(delivered)
        .expect("source sees delivery");
    source.delivered(sequence).expect("kernel sees delivery");

    assert_eq!(source.next_offer().expect("source completes"), None);
    assert_eq!(source.finish_kernel().expect("kernel terminal"), 1);
}

#[test]
fn observed_boot_rebinding_rejects_the_stale_planned_session() {
    let mut source =
        PicoWifiBootstrapSource::prepare(b"network", b"secret").expect("source prepares");
    let planned = source.binding().clone();
    source
        .observe_sink_boot(BootId::from("observed-pico-network-boot"))
        .expect("runtime boot rebinds");

    assert_eq!(
        source.admit_inbound(planned.hello_frame()),
        Err(format!("{:?}", WireError::InvalidSession))
    );

    let binding = source.binding().clone();
    let mut message = binding.hello_frame().message;
    if let SessionMessage::Hello(ref mut hello) = message {
        hello.base_instance_id = "wrong-usb-line";
    }
    assert_eq!(
        source.admit_inbound(SessionFrame {
            identity: binding.identity(),
            message,
        }),
        Err(format!("{:?}", WireError::InvalidSession))
    );
}

#[test]
fn malformed_or_oversized_credentials_fail_without_echoing_secret_bytes() {
    for result in [
        PicoWifiBootstrapSource::prepare(b"", b"do-not-echo"),
        PicoWifiBootstrapSource::prepare(
            b"network",
            &[b'x'; conduit_net::MAXIMUM_CREDENTIAL_BYTES + 1],
        ),
    ] {
        let error = match result {
            Ok(_) => panic!("invalid credentials must fail"),
            Err(error) => error,
        };
        assert!(!error.contains("do-not-echo"));
        assert!(!error.contains(&"x".repeat(conduit_net::MAXIMUM_CREDENTIAL_BYTES + 1)));
    }
}
