//! Bounded post-panic evidence and exact-build BOOTSEL recovery.

use crate::panic_recovery::PanicRecord;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc, UsbSignError};
use crate::usb_link::UsbLinkSession;

pub async fn serve(
    link: &mut UsbLinkSession,
    sign: &mut UsbCdc,
    record: PanicRecord,
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    if crate::wifi_join::establish_usb(link, sign, runtime)
        .await
        .is_ok()
    {
        let mut frame = [0_u8; 1024];
        loop {
            match link.receive_raw_stream_frame(&mut frame).await {
                Ok(raw) if raw == conduit_rp2040_network_realization::R1_USB_NETWORK_SESSION_QUERY => {
                    // End the host's CDC 0 readiness wait before writing the
                    // authoritative typed failure Sign on CDC 1.
                    if link
                        .send_raw_stream_frame(conduit_rp2040_network_realization::R1_USB_NETWORK_SESSION_FAILED)
                        .await
                        .is_ok()
                    {
                        let ready = link.receive_raw_stream_frame(&mut frame).await;
                        if !matches!(
                            ready,
                            Ok(raw) if raw == conduit_rp2040_network_realization::R1_USB_NETWORK_FAILURE_SIGN_READY
                        ) {
                            break;
                        }
                        crate::panic_recovery::set_phase(
                            crate::panic_recovery::PanicPhase::RecoverySign,
                        );
                        crate::panic_recovery::set_phase(
                            crate::panic_recovery::PanicPhase::RecoverySignWrite,
                        );
                        let status = match sign
                            .write_network_recovery_failure(
                                record.code(),
                                crate::wifi_join::attachment_identity(runtime),
                            )
                            .await
                        {
                            Ok(()) => conduit_rp2040_network_realization::R1_USB_NETWORK_FAILURE_SIGN_WRITTEN,
                            Err(UsbSignError::FormatOverflow) => {
                                conduit_rp2040_network_realization::R1_USB_NETWORK_FAILURE_SIGN_FORMAT_FAILED
                            }
                            Err(UsbSignError::Disconnected) => {
                                conduit_rp2040_network_realization::R1_USB_NETWORK_FAILURE_SIGN_DISCONNECTED
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
