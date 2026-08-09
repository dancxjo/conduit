//! Bounded UsbCdc remote session sink for Pico W signal presentation.
//!
//! Evaluates incoming SessionFrames, manages SessionMachine lifecycle,
//! ingests Signal items, drives CYW43 LED presentation, emits receipts over
//! CDC 1 (clue interface), and returns session truth on CDC 0.

use conduit_core::ConnectionBase;
#[cfg(not(feature = "wifi-bootstrap"))]
use conduit_core::{
    bind_active_play, BootId, ConnectionBaseInstanceId, ConnectionId, FragmentId, HostId, KindId,
    LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_wire::{SessionBinding, SessionMachine, SessionMessage, SessionTerminalDisposition};
#[cfg(not(feature = "wifi-bootstrap"))]
use conduit_wire::{RouteAttachment, SessionEndpointIdentity, SessionLimits, SessionRole};
use cyw43::Control;

#[cfg(not(feature = "wifi-bootstrap"))]
use crate::kernel::boot_identity;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
#[cfg(feature = "wifi-bootstrap")]
use crate::continuable_signal::ContinuableSignalSink;
use crate::remote_kernel::RemoteSignalKernel;
use crate::signal_execution_identity::SignalExecutionIdentity;
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::signal_image::generated_remote_endpoint;
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::signal_image::RemoteEndpointIdentity;
use crate::usb_link::{UsbLinkError, UsbLinkSession};

#[cfg(feature = "wifi-bootstrap")]
mod continuation;
#[cfg(feature = "wifi-bootstrap")]
pub use continuation::resume_plan_c_signal_sink;

/// Establish the two physical USB channels before platform initialization can
/// delay the session service loop.
#[cfg(not(feature = "wifi-bootstrap"))]
pub async fn establish_usb_channels(
    link_session: &mut UsbLinkSession,
    clue_cdc: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let mut frame_buf = [0u8; 2048];

    // Prove CDC 0 in both directions before touching CDC 1. The link must not be
    // held behind clue-channel startup or DTR state.
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

    // The boot identity is mandatory clue, but CDC 1 readiness must not
    // prevent the independent CDC 0 physical checkpoint above from completing.
    clue_cdc.wait_dtr().await;
    clue_cdc
        .write_boot_identity(boot_identity(), runtime)
        .await?;

    Ok(())
}

#[cfg(not(feature = "wifi-bootstrap"))]
pub async fn run_remote_signal_sink(
    link_session: &mut UsbLinkSession,
    clue_cdc: &mut UsbCdc,
    control: &mut Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let planned = generated_remote_endpoint().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    run_remote_signal_sink_for(
        link_session,
        clue_cdc,
        control,
        runtime,
        planned,
        SignalExecutionIdentity::plan_a(),
    )
    .await
}

#[cfg(feature = "wifi-bootstrap")]
pub async fn run_plan_b_signal_sink(
    link_session: &mut UsbLinkSession,
    clue_cdc: &mut UsbCdc,
    control: &mut Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
    state: &mut ContinuableSignalSink,
) -> Result<(), UsbLinkError> {
    let expected = SignalExecutionIdentity::plan_b();
    if state.identity.plan_id != expected.plan_id
        || state.identity.fragment_id != expected.fragment_id
        || state.identity.host_id != expected.host_id
        || state.binding().attachment.base != ConnectionBase::UsbCdc
    {
        return Err(UsbLinkError::InvalidGeneratedEndpoint);
    }
    let binding = &state.binding;
    run_prepared_signal_sink(
        link_session,
        clue_cdc,
        control,
        runtime,
        binding,
        &mut state.machine,
        &mut state.kernel,
        state.identity,
    )
    .await
}

#[cfg(not(feature = "wifi-bootstrap"))]
async fn run_remote_signal_sink_for(
    link_session: &mut UsbLinkSession,
    clue_cdc: &mut UsbCdc,
    control: &mut Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
    planned: RemoteEndpointIdentity,
    identity: SignalExecutionIdentity,
) -> Result<(), UsbLinkError> {
    let base = ConnectionBase::from_canonical_code(planned.base_code)
        .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    if base != ConnectionBase::UsbCdc
        || planned.local_host != identity.host_id
        || planned.local_boot != identity.boot_id
        || planned.sink_fragment_id != identity.fragment_id
    {
        return Err(UsbLinkError::InvalidGeneratedEndpoint);
    }
    let plan_id = PlanId::from(identity.plan_id);
    let source_host_id = HostId::from(planned.peer_host);
    let source_boot_id = BootId::from(planned.peer_boot);
    let sink_host_id = HostId::from(planned.local_host);
    let sink_boot_id = BootId::from(planned.local_boot);

    let source_active_play_id =
        bind_active_play(&plan_id, &source_host_id, &source_boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&plan_id, &sink_host_id, &sink_boot_id, 0).active_play_id;

    let binding = SessionBinding {
        protocol_version: 1,
        plan_id,
        source_fragment_id: FragmentId::from(planned.source_fragment_id),
        sink_fragment_id: FragmentId::from(planned.sink_fragment_id),
        source_active_play_id,
        sink_active_play_id,
        connection_id: ConnectionId::from(planned.connection_id),
        source: SessionEndpointIdentity {
            host_id: source_host_id.clone(),
            boot_id: source_boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host_id.clone(),
            boot_id: sink_boot_id.clone(),
        },
        value_kind: KindId::from(planned.value_kind),
        limits: SessionLimits {
            maximum_in_flight_items: planned.session_item_capacity,
            maximum_payload_bytes: planned.session_byte_capacity,
            maximum_buffered_bytes: planned.session_byte_capacity,
        },
        attachment: RouteAttachment {
            link_binding_id: LinkBindingId::from(planned.link_binding_id),
            base,
            base_instance_id: ConnectionBaseInstanceId::from(
                planned.base_instance_id,
            ),
            source_host_id,
            source_boot_id,
            source_endpoint_id: LinkEndpointId::from(planned.peer_endpoint),
            sink_host_id,
            sink_boot_id,
            sink_endpoint_id: LinkEndpointId::from(planned.local_endpoint),
            limits: LinkLimits {
                maximum_in_flight_items: planned.maximum_in_flight_items,
                maximum_payload_bytes: planned.maximum_payload_bytes,
                maximum_buffered_bytes: planned.maximum_buffered_bytes,
                maximum_frame_bytes: planned.maximum_frame_bytes,
            },
        },
    }
    .with_observed_boots(BootId::from(planned.peer_boot), BootId::from(runtime.boot_id()))
    .map_err(UsbLinkError::Codec)?;

    let mut machine =
        SessionMachine::new(binding.clone(), SessionRole::Sink).map_err(UsbLinkError::Codec)?;
    let mut kernel = RemoteSignalKernel::new(identity)?;

    run_prepared_signal_sink(
        link_session,
        clue_cdc,
        control,
        runtime,
        &binding,
        &mut machine,
        &mut kernel,
        identity,
    )
    .await
}

// Transport, platform effect, transcript, and already-admitted execution state
// remain separate collaborators at this boundary.
#[allow(clippy::too_many_arguments)]
async fn run_prepared_signal_sink(
    link_session: &mut UsbLinkSession,
    clue_cdc: &mut UsbCdc,
    control: &mut Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
    binding: &SessionBinding,
    machine: &mut SessionMachine,
    kernel: &mut RemoteSignalKernel,
    identity: SignalExecutionIdentity,
) -> Result<(), UsbLinkError> {
    let mut frame_buf = [0u8; 2048];
    let mut failure_disposition: Option<SessionTerminalDisposition> = None;

    // Phase 1: SessionMessage::Hello wait on the proven CDC 0 path.
    let raw_bytes = link_session.receive_raw_stream_frame(&mut frame_buf).await?;
    let frame = conduit_wire::decode_session_frame(raw_bytes, 1024, 1024)?;
    if !matches!(frame.message, SessionMessage::Hello(_)) {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }
    machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
    let hello_frame = binding.hello_frame();
    machine.admit_outbound(hello_frame).map_err(UsbLinkError::Codec)?;
    link_session.send_frame(&hello_frame).await?;

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
            let admitted = match kernel.admit(sequence, payload) {
                Ok(admitted) => admitted,
                Err(error) => {
                    return fail_active_session(
                        link_session,
                        binding,
                        machine,
                        kernel,
                        error,
                    )
                    .await;
                }
            };
            match admitted {
                RemoteIngressOutcome::Full { .. } => {
                    let pressure = binding.frame(SessionMessage::Pressure { sequence });
                    machine
                        .admit_outbound(pressure)
                        .map_err(UsbLinkError::Codec)?;
                    link_session.send_frame(&pressure).await?;
                    continue;
                }
                RemoteIngressOutcome::Accepted { .. } => {}
            }

            // Accepted is emitted only after the generated remote cord owns the value.
            let accepted_frame = binding.frame(SessionMessage::Accepted { sequence });
            machine
                .admit_outbound(accepted_frame)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&accepted_frame).await?;

            // Delivered is emitted only after the kernel-owned host operation has
            // completed the physical LED effect and its mandatory receipt.
            if let Err(error) = kernel
                .present_accepted(sequence, control, clue_cdc, runtime)
                .await
            {
                return fail_active_session(
                    link_session,
                    binding,
                    machine,
                    kernel,
                    error,
                )
                .await;
            }

            let delivered_frame = binding.frame(SessionMessage::Delivered { sequence });
            machine
                .admit_outbound(delivered_frame)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&delivered_frame).await?;
        } else if let SessionMessage::InputClosed { final_sequence } = frame.message {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
            kernel.close_and_complete(final_sequence)?;
        } else if let SessionMessage::Terminal {
            disposition,
            final_sequence,
        } = frame.message
        {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
            if failure_disposition.is_some_and(|expected| expected != disposition) {
                return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
            }
            let terminal = binding.frame(SessionMessage::Terminal {
                disposition,
                final_sequence,
            });
            machine
                .admit_outbound(terminal)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&terminal).await?;
        } else if let SessionMessage::Cancelled { code } = frame.message {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
            kernel.cancel()?;
            failure_disposition = Some(SessionTerminalDisposition::Cancelled);
            let response = binding.frame(SessionMessage::Cancelled { code });
            machine
                .admit_outbound(response)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&response).await?;
        } else if let SessionMessage::Failed { code } = frame.message {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
            kernel.cancel()?;
            failure_disposition = Some(SessionTerminalDisposition::Failed);
            let response = binding.frame(SessionMessage::Failed { code });
            machine
                .admit_outbound(response)
                .map_err(UsbLinkError::Codec)?;
            link_session.send_frame(&response).await?;
        } else {
            machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
        }

        if machine.is_terminal() {
            break;
        }
    }

    if failure_disposition.is_some() {
        return Err(UsbLinkError::KernelCancelled);
    }
    clue_cdc
        .write_terminal(true, identity.terminal(), runtime)
        .await?;

    Ok(())
}

async fn fail_active_session(
    link_session: &mut UsbLinkSession,
    binding: &SessionBinding,
    machine: &mut SessionMachine,
    kernel: &mut RemoteSignalKernel,
    cause: UsbLinkError,
) -> Result<(), UsbLinkError> {
    const SINK_FAILURE_CODE: u16 = 9;

    kernel.cancel()?;
    let failed = binding.frame(SessionMessage::Failed {
        code: SINK_FAILURE_CODE,
    });
    machine
        .admit_outbound(failed)
        .map_err(UsbLinkError::Codec)?;
    link_session.send_frame(&failed).await?;

    let mut frame_buf = [0_u8; 2048];
    let peer_failed = link_session.receive_frame(&mut frame_buf).await?;
    if !matches!(
        peer_failed.message,
        SessionMessage::Failed {
            code: SINK_FAILURE_CODE
        }
    ) {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }
    machine
        .admit_inbound(peer_failed)
        .map_err(UsbLinkError::Codec)?;

    let final_sequence = machine.next_sequence();
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Failed,
        final_sequence,
    });
    machine
        .admit_outbound(terminal)
        .map_err(UsbLinkError::Codec)?;
    link_session.send_frame(&terminal).await?;

    let peer_terminal = link_session.receive_frame(&mut frame_buf).await?;
    machine
        .admit_inbound(peer_terminal)
        .map_err(UsbLinkError::Codec)?;
    if !matches!(
        peer_terminal.message,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Failed,
            final_sequence: peer_final,
        } if peer_final == final_sequence
    ) || !machine.is_terminal()
    {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }

    Err(cause)
}
