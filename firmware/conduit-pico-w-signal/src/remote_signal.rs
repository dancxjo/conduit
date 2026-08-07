//! Bounded UsbCdc remote session sink for Pico W signal presentation.
//!
//! Evaluates incoming SessionFrames, manages SessionMachine lifecycle,
//! ingests Signal items, drives CYW43 LED presentation, emits receipts over
//! CDC 1 (evidence interface), and returns session truth on CDC 0.

use conduit_core::{
    bind_active_play, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_signal::decode_signal_bytes;
use conduit_wire::{SessionBinding, SessionMachine, SessionMessage, SessionRole};
use cyw43::Control;

use crate::kernel::{boot_identity, presentation_receipt_identity, terminal_identity};
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::signal_image::{presentation_identity, FRAGMENT_ID, HOST_ID, PLAN_ID};
use crate::usb_link::{UsbLinkError, UsbLinkSession};

/// Establish the two physical USB channels before platform initialization can
/// delay the session service loop.
pub async fn establish_usb_channels(
    link_session: &mut UsbLinkSession,
    evidence_cdc: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let mut frame_buf = [0u8; 2048];

    // Prove CDC 0 in both directions before touching CDC 1. The link must not be
    // held behind evidence-channel startup or DTR state.
    link_session.wait_connection().await;
    loop {
        let raw_bytes = link_session
            .receive_raw_stream_frame(&mut frame_buf)
            .await?;
        if crate::bootsel::handle_request(link_session, raw_bytes).await? {
            continue;
        }
        if raw_bytes == b"CONDUIT_RAW_CDC0_PROBE" {
            link_session
                .send_raw_stream_frame(b"CONDUIT_RAW_CDC0_REPLY")
                .await?;
            break;
        }
    }

    // The boot identity is mandatory evidence, but CDC 1 readiness must not
    // prevent the independent CDC 0 physical checkpoint above from completing.
    evidence_cdc.wait_dtr().await;
    evidence_cdc
        .write_boot_identity(boot_identity(), runtime)
        .await?;

    Ok(())
}

pub async fn run_remote_signal_sink(
    link_session: &mut UsbLinkSession,
    evidence_cdc: &mut UsbCdc,
    control: &mut Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let plan_id = PlanId::from(PLAN_ID);
    let source_host_id = HostId::from("host/std");
    let source_boot_id = BootId::from("boot/std");
    let sink_host_id = HostId::from(HOST_ID);
    let sink_boot_id = BootId::from(runtime.boot_id());

    let source_active_play_id =
        bind_active_play(&plan_id, &source_host_id, &source_boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&plan_id, &sink_host_id, &sink_boot_id, 0).active_play_id;

    let binding = SessionBinding {
        protocol_version: 1,
        plan_id,
        source_fragment_id: FragmentId::from("fragment/std-source"),
        sink_fragment_id: FragmentId::from(FRAGMENT_ID),
        source_active_play_id,
        sink_active_play_id,
        connection_id: ConnectionId::from("conn/std-pico-signal"),
        link_binding_id: LinkBindingId::from("link/usb-cdc-0"),
        provider: ConnectionProvider::UsbCdc,
        provider_instance_id: ConnectionProviderInstanceId::from("pico-usb-cdc-0"),
        source: LinkEndpoint {
            host_id: source_host_id,
            boot_id: source_boot_id,
            endpoint_id: LinkEndpointId::from("endpoint/std-out"),
        },
        sink: LinkEndpoint {
            host_id: sink_host_id,
            boot_id: sink_boot_id,
            endpoint_id: LinkEndpointId::from("endpoint/pico-in"),
        },
        value_kind: KindId::from("value/signal"),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 1024,
            maximum_frame_bytes: 1024,
        },
    };

    let mut machine =
        SessionMachine::new(binding.clone(), SessionRole::Sink).map_err(UsbLinkError::Codec)?;

    let mut frame_buf = [0u8; 2048];

    // Phase 1: SessionMessage::Hello wait on the proven CDC 0 path.
    loop {
        let raw_bytes = link_session.receive_raw_stream_frame(&mut frame_buf).await?;
        let frame = conduit_wire::decode_session_frame(raw_bytes, 1024, 1024)?;
        if !matches!(frame.message, SessionMessage::Hello(_)) {
            return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
        }
        machine
            .admit_inbound(frame)
            .map_err(UsbLinkError::Codec)?;

        let hello_frame = binding.hello_frame();
        machine
            .admit_outbound(hello_frame)
            .map_err(UsbLinkError::Codec)?;
        link_session.send_frame(&hello_frame).await?;
        break;
    }

    // Phase 2: Receive SessionMessage::Ready from Source and emit SessionMessage::Ready from Sink
    let ready_inbound = link_session.receive_frame(&mut frame_buf).await?;
    if !matches!(ready_inbound.message, SessionMessage::Ready) {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }
    machine
        .admit_inbound(ready_inbound)
        .map_err(UsbLinkError::Codec)?;

    let ready_outbound = binding.frame(SessionMessage::Ready);
    machine
        .admit_outbound(ready_outbound)
        .map_err(UsbLinkError::Codec)?;
    link_session.send_frame(&ready_outbound).await?;

    if !machine.is_active() {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }

    // Phase 3: Main event loop - full Offered -> Accepted -> Delivered lifecycle
    loop {
        let frame = link_session.receive_frame(&mut frame_buf).await?;

        if let SessionMessage::Offered { sequence, payload } = frame.message {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;

            // Admit outbound Accepted frame and send over CDC 0
            let accepted_frame = binding.frame(SessionMessage::Accepted { sequence });
            machine
                .admit_outbound(accepted_frame)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&accepted_frame).await?;

            // Execute hardware effect (LED toggle + evidence receipt)
            if let Ok(signal) = decode_signal_bytes(payload) {
                if let Some(identity) = presentation_identity(signal.sequence as usize) {
                    control.gpio_set(0, signal.level).await;
                    evidence_cdc
                        .write_receipt(
                            signal.sequence,
                            signal.level,
                            presentation_receipt_identity(identity),
                            runtime,
                        )
                        .await?;
                }
            }

            // Admit outbound Delivered frame and send over CDC 0
            let delivered_frame = binding.frame(SessionMessage::Delivered { sequence });
            machine
                .admit_outbound(delivered_frame)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&delivered_frame).await?;
        } else if let SessionMessage::Terminal {
            disposition,
            final_sequence,
        } = frame.message
        {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
            let terminal = binding.frame(SessionMessage::Terminal {
                disposition,
                final_sequence,
            });
            machine
                .admit_outbound(terminal)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&terminal).await?;
        } else {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
        }

        if machine.is_terminal() {
            break;
        }
    }

    evidence_cdc
        .write_terminal(true, terminal_identity(), runtime)
        .await?;

    Ok(())
}
