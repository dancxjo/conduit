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
use conduit_wire::{SessionBinding, SessionFrame, SessionMachine, SessionMessage, SessionRole};
use cyw43::Control;

use crate::kernel::{boot_identity, presentation_receipt_identity};
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::signal_image::{presentation_identity, FRAGMENT_ID, HOST_ID, PLAN_ID};
use crate::usb_link::{UsbLinkError, UsbLinkSession};

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

    let session_identity = binding.identity();

    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(UsbLinkError::Codec)?;

    let mut frame_buf = [0u8; 2048];

    // Phase 1: Wait for host to connect by repeatedly emitting the boot identity
    // and waiting for the Hello frame to arrive.
    loop {
        evidence_cdc.write_boot_identity(boot_identity(), runtime).await;

        match embassy_time::with_timeout(
            embassy_time::Duration::from_millis(500),
            link_session.receive_frame(&mut frame_buf),
        )
        .await
        {
            Ok(Ok(frame)) => {
                let is_hello = matches!(frame.message, SessionMessage::Hello(_));
                if machine.admit_inbound(frame.clone()).is_err() {
                    break;
                }

                if is_hello {
                    let hello_frame = binding.hello_frame();
                    if machine.admit_outbound(hello_frame.clone()).is_ok() {
                        let _ = link_session.send_frame(&hello_frame).await;
                    }

                    let ready_frame = SessionFrame {
                        identity: session_identity,
                        message: SessionMessage::Ready,
                    };
                    if machine.admit_outbound(ready_frame.clone()).is_ok() {
                        let _ = link_session.send_frame(&ready_frame).await;
                    }
                    break;
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Timeout, loop to emit boot identity again
                continue;
            }
        }
    }

    // Phase 2: Main event loop
    loop {
        let frame = link_session.receive_frame(&mut frame_buf).await?;
        
        if machine.admit_inbound(frame.clone()).is_err() {
            break;
        }

        if let SessionMessage::Offered { payload, .. } = frame.message {
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
                        .await;
                }
            }
        }

        if machine.is_terminal() {
            break;
        }
    }

    Ok(())
}
