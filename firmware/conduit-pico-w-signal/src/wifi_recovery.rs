//! Bounded post-panic evidence and exact-build BOOTSEL recovery.

use crate::panic_recovery::PanicRecord;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc, UsbClueError};
use crate::usb_link::UsbLinkSession;

pub async fn serve(
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    record: PanicRecord,
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    if crate::wifi_join::establish_usb(link, clue, runtime)
        .await
        .is_ok()
    {
        let mut frame = [0_u8; 1024];
        loop {
            match link.receive_raw_stream_frame(&mut frame).await {
                Ok(raw) if raw == conduit_net::R1_USB_NETWORK_SESSION_QUERY => {
                    // End the host's CDC 0 readiness wait before writing the
                    // authoritative typed failure Clue on CDC 1.
                    if link
                        .send_raw_stream_frame(conduit_net::R1_USB_NETWORK_SESSION_FAILED)
                        .await
                        .is_ok()
                    {
                        let ready = link.receive_raw_stream_frame(&mut frame).await;
                        if !matches!(
                            ready,
                            Ok(raw) if raw == conduit_net::R1_USB_NETWORK_FAILURE_CLUE_READY
                        ) {
                            break;
                        }
                        crate::panic_recovery::set_phase(
                            crate::panic_recovery::PanicPhase::RecoveryClue,
                        );
                        crate::panic_recovery::set_phase(
                            crate::panic_recovery::PanicPhase::RecoveryClueWrite,
                        );
                        let status = match clue
                            .write_network_failure(
                                record.code(),
                                crate::wifi_join::attachment_identity(runtime),
                            )
                            .await
                        {
                            Ok(()) => conduit_net::R1_USB_NETWORK_FAILURE_CLUE_WRITTEN,
                            Err(UsbClueError::FormatOverflow) => {
                                conduit_net::R1_USB_NETWORK_FAILURE_CLUE_FORMAT_FAILED
                            }
                            Err(UsbClueError::Disconnected) => {
                                conduit_net::R1_USB_NETWORK_FAILURE_CLUE_DISCONNECTED
                            }
                        };
                        let _ = link.send_raw_stream_frame(status).await;
                    }
                    break;
                }
                Ok(raw) => {
                    let _ = crate::bootsel::handle_request(link, raw).await;
                }
                Err(_) => break,
            }
        }
    }
    loop {
        crate::bootsel::wait_for_request(link).await.ok();
    }
}
