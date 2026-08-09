//! USB continuation of the retained dual-Line Plan C sink.

use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_wire::{
    decode_session_checkpoint, encode_session_checkpoint_into, SessionMessage,
    SessionTerminalDisposition,
};

use crate::continuable_signal::ContinuableSignalSink;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::{UsbLinkError, UsbLinkSession};

pub async fn resume_plan_c_signal_sink(
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
    state: &mut ContinuableSignalSink,
) -> Result<(), UsbLinkError> {
    let mut peer_bytes = [0_u8; 2048];
    let peer_raw = link.receive_raw_stream_frame(&mut peer_bytes).await?;
    let peer = decode_session_checkpoint(peer_raw, 2048)?;

    let mut local_bytes = [0_u8; 2048];
    let local_len = encode_session_checkpoint_into(
        state.machine.checkpoint_offer(),
        &mut local_bytes,
        2048,
    )?;
    link.send_raw_stream_frame(&local_bytes[..local_len]).await?;

    let acceptance = state.resume_usb(runtime, peer)?;
    if !acceptance.same_plan_continues {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidSession));
    }
    let binding = &state.binding;
    let mut frame_buf = [0_u8; 2048];

    let hello = link.receive_frame(&mut frame_buf).await?;
    if !matches!(hello.message, SessionMessage::Hello(_)) {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }
    state.machine.admit_inbound(hello).map_err(UsbLinkError::Codec)?;
    let response = binding.hello_frame();
    state.machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
    link.send_frame(&response).await?;

    let ready = link.receive_frame(&mut frame_buf).await?;
    if !matches!(ready.message, SessionMessage::Ready) {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }
    state.machine.admit_inbound(ready).map_err(UsbLinkError::Codec)?;
    let response = binding.frame(SessionMessage::Ready);
    state.machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
    link.send_frame(&response).await?;
    if !state.machine.is_active() {
        return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
    }

    let mut failure = None;
    loop {
        let frame = link.receive_frame(&mut frame_buf).await?;
        match frame.message {
            SessionMessage::Offered { sequence, payload } => {
                state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
                match state.kernel.admit(sequence, payload)? {
                    RemoteIngressOutcome::Full { .. } => {
                        let pressure = binding.frame(SessionMessage::Pressure { sequence });
                        state.machine.admit_outbound(pressure).map_err(UsbLinkError::Codec)?;
                        link.send_frame(&pressure).await?;
                        continue;
                    }
                    RemoteIngressOutcome::Accepted { .. } => {}
                }
                let accepted = binding.frame(SessionMessage::Accepted { sequence });
                state.machine.admit_outbound(accepted).map_err(UsbLinkError::Codec)?;
                link.send_frame(&accepted).await?;
                state.kernel.present_accepted(sequence, control, clue, runtime).await?;
                let delivered = binding.frame(SessionMessage::Delivered { sequence });
                state.machine.admit_outbound(delivered).map_err(UsbLinkError::Codec)?;
                link.send_frame(&delivered).await?;
            }
            SessionMessage::InputClosed { final_sequence } => {
                state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
                state.kernel.close_and_complete(final_sequence)?;
            }
            SessionMessage::Terminal { disposition, final_sequence } => {
                state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
                if failure.is_some_and(|expected| expected != disposition) {
                    return Err(UsbLinkError::Codec(conduit_wire::WireError::InvalidState));
                }
                let terminal = binding.frame(SessionMessage::Terminal {
                    disposition,
                    final_sequence,
                });
                state.machine.admit_outbound(terminal).map_err(UsbLinkError::Codec)?;
                link.send_frame(&terminal).await?;
            }
            SessionMessage::Cancelled { code } => {
                state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
                state.kernel.cancel()?;
                failure = Some(SessionTerminalDisposition::Cancelled);
                let response = binding.frame(SessionMessage::Cancelled { code });
                state.machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
                link.send_frame(&response).await?;
            }
            SessionMessage::Failed { code } => {
                state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?;
                state.kernel.cancel()?;
                failure = Some(SessionTerminalDisposition::Failed);
                let response = binding.frame(SessionMessage::Failed { code });
                state.machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
                link.send_frame(&response).await?;
            }
            _ => state.machine.admit_inbound(frame).map_err(UsbLinkError::Codec)?,
        }
        if state.machine.is_terminal() {
            break;
        }
    }
    if failure.is_some() {
        return Err(UsbLinkError::KernelCancelled);
    }
    clue.write_terminal(true, state.identity.terminal(), runtime).await?;
    Ok(())
}
