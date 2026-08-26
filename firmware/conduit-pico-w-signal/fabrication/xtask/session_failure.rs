use std::time::Duration;

use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_std_host::usb_cdc::NativePathCdcLine;
use conduit_wire::{SessionBinding, SessionMessage, SessionTerminalDisposition};

use super::PicoResult;

pub(super) fn complete_induced_sink_failure(
    source: &mut PicoUsbSource,
    line: &mut NativePathCdcLine,
    binding: &SessionBinding,
    sequence: u64,
    payload: &[u8; conduit_signal::SIGNAL_ENCODED_LEN as usize],
) -> PicoResult<()> {
    const SINK_FAILURE_CODE: u16 = 9;

    let offer = binding.frame(SessionMessage::Offered { sequence, payload });
    source.admit_outbound(offer)?;
    line.send_frame(&offer, Duration::from_secs(2))?;

    let mut frame_buf = [0_u8; 2048];
    let accepted = line.receive_frame(&mut frame_buf, Duration::from_secs(2))?;
    if !matches!(
        accepted.message,
        SessionMessage::Accepted {
            sequence: accepted_sequence
        } if accepted_sequence == sequence
    ) {
        return Err(format!(
            "expected ownership Accepted before sink failure, received {:?}",
            accepted.message
        )
        .into());
    }
    source.admit_inbound(accepted)?;
    source.accepted(sequence)?;

    let failed = line.receive_frame(&mut frame_buf, Duration::from_secs(2))?;
    if !matches!(
        failed.message,
        SessionMessage::Failed {
            code: SINK_FAILURE_CODE
        }
    ) {
        return Err(format!("expected sink Failed, received {:?}", failed.message).into());
    }
    source.admit_inbound(failed)?;
    source.cancel()?;

    let response = binding.frame(SessionMessage::Failed {
        code: SINK_FAILURE_CODE,
    });
    source.admit_outbound(response)?;
    line.send_frame(&response, Duration::from_secs(2))?;

    let terminal = line.receive_frame(&mut frame_buf, Duration::from_secs(2))?;
    let terminal_response = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Failed,
        final_sequence: sequence,
    });
    if !matches!(
        terminal.message,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Failed,
            final_sequence: peer_final,
        } if peer_final == sequence
    ) {
        return Err(format!(
            "expected sink failed terminal, received {:?}",
            terminal.message
        )
        .into());
    }
    source.admit_inbound(terminal)?;
    source.admit_outbound(terminal_response)?;
    line.send_frame(&terminal_response, Duration::from_secs(2))?;
    if !source.is_terminal() {
        return Err("source did not reach failed terminal agreement".into());
    }
    Ok(())
}
