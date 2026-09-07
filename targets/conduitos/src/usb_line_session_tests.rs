use super::*;
use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId, HostId, KindId, LineId,
    LinkBindingId, LinkEndpointId, LinkLimits, PlanId, bind_active_play,
};
use conduit_wire::{LineAttachment, SessionEndpointIdentity, SessionLimits};

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("plan/usb-line");
    let source_host = HostId::from("host/source");
    let source_boot = BootId::from("boot/source");
    let sink_host = HostId::from("host/sink");
    let sink_boot = BootId::from("boot/sink");
    SessionBinding {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        source_fragment_id: FragmentId::from("fragment/source"),
        sink_fragment_id: FragmentId::from("fragment/sink"),
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        connection_id: ConnectionId::from("connection/usb-line"),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from("info/text@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: USB_LINE_MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: USB_LINE_MAXIMUM_FRAME_BYTES,
        },
        attachment: LineAttachment {
            line_id: LineId::from("line/usb-ftdi"),
            link_binding_id: LinkBindingId::from("binding/usb-ftdi"),
            base: BaseImplementationId::from(crate::usb_line_offer::USB_FTDI_BASE),
            base_instance_id: BaseInstanceId::from("base/usb-ftdi/1"),
            contract: crate::usb_line_offer::usb_ftdi_contract(),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from("endpoint/source"),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: USB_LINE_MAXIMUM_PAYLOAD_BYTES,
                maximum_buffered_bytes: USB_LINE_MAXIMUM_FRAME_BYTES,
                maximum_frame_bytes: USB_LINE_MAXIMUM_FRAME_BYTES,
            },
        },
    }
}

#[test]
fn canonical_binding_fits_the_fixed_usb_stream_storage() {
    let session = UsbLineSession::new(binding(), SessionRole::Source).unwrap();
    assert_eq!(session.binding().plan_id.as_str(), "plan/usb-line");
    assert!(!session.is_active());
    assert!(core::mem::size_of::<UsbLineSession>() < 8_192);
}

#[test]
fn stream_header_is_incremental_bounded_and_nonempty() {
    let mut bytes = [0; USB_LINE_STREAM_BYTES];
    assert_eq!(complete_frame_length(&bytes, 0), Ok(None));
    bytes[..2].copy_from_slice(&7_u16.to_be_bytes());
    assert_eq!(complete_frame_length(&bytes, 2), Ok(None));
    assert_eq!(complete_frame_length(&bytes, 9), Ok(Some(7)));

    bytes[..2].copy_from_slice(&0_u16.to_be_bytes());
    assert!(matches!(
        complete_frame_length(&bytes, 2),
        Err(UsbLineSessionError::Framing(
            StreamFrameError::ZeroLengthFrame
        ))
    ));
}

#[test]
fn received_payload_is_copied_into_fixed_owned_storage() {
    let received = ReceivedSessionMessage::copy_from(SessionMessage::Offered {
        sequence: 7,
        payload: b"hello",
    })
    .unwrap();
    let ReceivedSessionMessage::Offered {
        sequence,
        payload,
        length,
    } = received
    else {
        panic!("wrong message");
    };
    assert_eq!(sequence, 7);
    assert_eq!(&payload[..usize::from(length)], b"hello");
}
