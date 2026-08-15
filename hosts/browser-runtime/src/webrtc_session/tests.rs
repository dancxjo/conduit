use super::start::conduit_browser_webrtc_session_start_granted;
use super::*;
use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LineId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{LineAttachment, SessionEndpointIdentity, SessionLimits};

fn dynamic_binding() -> SessionBinding {
    let plan_id = PlanId::from("body-grant/dynamic-plan");
    let source_host_id = HostId::from("browser/dynamic-source");
    let source_boot_id = BootId::from("browser-boot/dynamic-source");
    let sink_host_id = HostId::from("browser/dynamic-sink");
    let sink_boot_id = BootId::from("browser-boot/dynamic-sink");
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(&plan_id, &source_host_id, &source_boot_id, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host_id, &sink_boot_id, 0)
            .active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from("body-grant/dynamic-source-fragment"),
        sink_fragment_id: FragmentId::from("body-grant/dynamic-sink-fragment"),
        connection_id: ConnectionId::from("body-grant/dynamic-connection"),
        source: SessionEndpointIdentity {
            host_id: source_host_id.clone(),
            boot_id: source_boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host_id.clone(),
            boot_id: sink_boot_id.clone(),
        },
        value_kind: KindId::from("conduit.test/dynamic-bounded-bytes@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: PAYLOAD_CAPACITY,
            maximum_buffered_bytes: PAYLOAD_CAPACITY,
        },
        attachment: LineAttachment {
            line_id: LineId::from("body-grant/dynamic-line"),
            link_binding_id: LinkBindingId::from("body-grant/dynamic-link"),
            base: ConnectionBase::WebRtcDataChannel,
            base_instance_id: ConnectionBaseInstanceId::from("body-grant/dynamic-base-instance"),
            source_host_id,
            source_boot_id,
            source_endpoint_id: LinkEndpointId::from("body-grant/dynamic-source-endpoint"),
            sink_host_id,
            sink_boot_id,
            sink_endpoint_id: LinkEndpointId::from("body-grant/dynamic-sink-endpoint"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: PAYLOAD_CAPACITY,
                maximum_buffered_bytes: PAYLOAD_CAPACITY,
                maximum_frame_bytes: FRAME_CAPACITY as u32,
            },
        },
    }
}

#[test]
fn exact_webrtc_binding_is_planned_and_session_eligible() {
    let binding = exact_binding(0).unwrap();
    assert_eq!(
        binding.attachment.base,
        conduit_core::ConnectionBase::WebRtcDataChannel
    );
    assert_eq!(binding.attachment.base.canonical_code(), 7);
    assert!(binding.attachment.base.supports_remote_session());
    assert!(SessionMachine::new(binding, SessionRole::Source).is_ok());
}

#[test]
fn granted_start_reconstructs_exact_binding_and_refuses_non_hello() {
    let binding = dynamic_binding();
    binding.validate().unwrap();
    let mut bytes = [0; FRAME_CAPACITY];
    let length = encode_session_frame_into(
        binding.hello_frame(),
        &mut bytes,
        PAYLOAD_CAPACITY,
        FRAME_CAPACITY as u32,
    )
    .unwrap();
    INPUT.with(|input| input.borrow_mut()[..length].copy_from_slice(&bytes[..length]));
    assert_eq!(
        conduit_browser_webrtc_session_start_granted(0, length as u32),
        STATUS_HANDSHAKE
    );
    ENDPOINT.with(|slot| {
        assert_eq!(slot.borrow().as_ref().unwrap().binding, binding);
    });

    let ready_length = encode_session_frame_into(
        binding.frame(SessionMessage::Ready),
        &mut bytes,
        PAYLOAD_CAPACITY,
        FRAME_CAPACITY as u32,
    )
    .unwrap();
    INPUT.with(|input| input.borrow_mut()[..ready_length].copy_from_slice(&bytes[..ready_length]));
    assert!(conduit_browser_webrtc_session_start_granted(0, ready_length as u32) < 0);
    ENDPOINT.with(|slot| assert!(slot.borrow().is_none()));
}

#[test]
fn out_of_stage_failure_does_not_mutate_or_create_false_active_state() {
    let binding = exact_binding(0).unwrap();
    let mut endpoint =
        BrowserWebRtcSession::new(SessionRole::Source, exact_binding(0).unwrap()).unwrap();
    endpoint.output_len = 0;

    let mut bytes = [0; FRAME_CAPACITY];
    let failed = binding.frame(SessionMessage::Failed { code: 1 });
    let failed_len =
        encode_session_frame_into(failed, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
            .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..failed_len]), Ok(ERROR_STAGE));
    assert_eq!(endpoint.stage, Stage::PeerHello);

    let hello = binding.hello_frame();
    let hello_len =
        encode_session_frame_into(hello, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
            .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..hello_len]), Ok(STATUS_HANDSHAKE));
    endpoint.output_len = 0;
    let ready = binding.frame(SessionMessage::Ready);
    let ready_len =
        encode_session_frame_into(ready, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
            .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..ready_len]), Ok(STATUS_ACTIVE));
    assert!(endpoint.machine.is_active());
}

#[test]
fn reordered_offer_refuses_without_consuming_the_expected_sequence() {
    let binding = exact_binding(0).unwrap();
    let mut endpoint =
        BrowserWebRtcSession::new(SessionRole::Sink, exact_binding(0).unwrap()).unwrap();
    endpoint.output_len = 0;

    let mut bytes = [0; FRAME_CAPACITY];
    let hello = binding.hello_frame();
    let hello_len =
        encode_session_frame_into(hello, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
            .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..hello_len]), Ok(STATUS_HANDSHAKE));
    endpoint.output_len = 0;
    let ready = binding.frame(SessionMessage::Ready);
    let ready_len =
        encode_session_frame_into(ready, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
            .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..ready_len]), Ok(STATUS_ACTIVE));

    let reordered = binding.frame(SessionMessage::Offered {
        sequence: 1,
        payload: &[7],
    });
    let reordered_len = encode_session_frame_into(
        reordered,
        &mut bytes,
        PAYLOAD_CAPACITY,
        FRAME_CAPACITY as u32,
    )
    .unwrap();
    assert_eq!(
        endpoint.ingest(&bytes[..reordered_len]),
        Err(WireError::ReorderedFrame)
    );
    assert_eq!(endpoint.machine.next_sequence(), 0);
    assert_eq!(endpoint.received_sequence, None);

    let expected = binding.frame(SessionMessage::Offered {
        sequence: 0,
        payload: &[7],
    });
    let expected_len = encode_session_frame_into(
        expected,
        &mut bytes,
        PAYLOAD_CAPACITY,
        FRAME_CAPACITY as u32,
    )
    .unwrap();
    assert_eq!(endpoint.ingest(&bytes[..expected_len]), Ok(STATUS_ACTIVE));
    assert_eq!(endpoint.received_sequence, Some(0));
    assert_eq!(&endpoint.received[..endpoint.received_len], &[7]);

    endpoint.output_len = 0;
    assert_eq!(
        endpoint.ingest(&bytes[..expected_len]),
        Err(WireError::ReorderedFrame)
    );
    assert_eq!(endpoint.machine.next_sequence(), 0);
    assert_eq!(endpoint.received_sequence, Some(0));
    assert_eq!(&endpoint.received[..endpoint.received_len], &[7]);
}
