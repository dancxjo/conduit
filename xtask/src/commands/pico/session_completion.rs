//! Exact terminal agreement for the std-to-Pico physical session.

use std::time::{Duration, Instant};

use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};
use conduit_wire::{SessionBinding, SessionMessage, SessionTerminalDisposition};

use super::firmware::FirmwareIdentity;
use super::transcript::{self, RuntimeTranscriptIdentity};
use super::PicoResult;

pub fn complete(
    source: &mut PicoUsbSource,
    carrier: &mut NativePathCdcCarrier,
    evidence: &mut NativePathCdcLineReader,
    binding: &SessionBinding,
    final_sequence: u64,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let input_closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source
        .admit_outbound(input_closed)
        .map_err(|error| format!("source rejected outbound InputClosed: {error:?}"))?;
    carrier.send_frame(&input_closed, Duration::from_secs(2))?;

    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence,
    });
    source
        .admit_outbound(terminal)
        .map_err(|error| format!("source rejected outbound Terminal: {error:?}"))?;
    carrier.send_frame(&terminal, Duration::from_secs(2))?;

    let mut frame_buf = [0_u8; 2048];
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if Instant::now() >= deadline {
            return Err("timed out waiting for Pico terminal agreement".into());
        }
        match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
            Ok(frame) => match frame.message {
                SessionMessage::Terminal {
                    disposition: SessionTerminalDisposition::Completed,
                    final_sequence: peer_final,
                } if peer_final == final_sequence => {
                    source.admit_inbound(frame).map_err(|error| {
                        format!("source rejected inbound Pico Terminal: {error:?}")
                    })?;
                    break;
                }
                other => return Err(format!("unexpected Pico terminal response: {other:?}").into()),
            },
            Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
            Err(error) => {
                return Err(format!("failed receiving Pico terminal response: {error:?}").into())
            }
        }
    }

    if !source.is_terminal() {
        return Err("source session did not reach exact terminal agreement".into());
    }
    let line = evidence
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading Pico terminal evidence: {error}"))?;
    transcript::verify_terminal(&line, identity, runtime)?;
    println!("==> Pico terminal agreement and CDC 1 terminal evidence validated");
    Ok(())
}
