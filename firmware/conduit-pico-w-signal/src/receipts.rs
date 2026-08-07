//! USB CDC receipt emission for machine-readable Pico W signal proof.
//!
//! Emits newline-delimited JSON receipt records over a USB serial port.
//! The verifier (xtask pico verify) reads these records to confirm the
//! exact Signal sequence, levels, and terminal disposition.

use core::fmt::Write as _;
use embassy_rp::clocks::RoscRng;
use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_time::Instant;
use embassy_usb::UsbDevice;
use heapless::String as HString;

use crate::usb::MAX_PACKET_SIZE;

// The longest current identity-bearing receipt is about 1.6 KiB. This fixed
// bound leaves room for longer exact runtime IDs without allocating.
const RECEIPT_BUFFER_BYTES: usize = 2048;
const RUNTIME_ID_BYTES: usize = 128;

pub struct UsbCdc {
    sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbEvidenceError {
    FormatOverflow,
    Disconnected,
}

impl UsbCdc {
    pub fn new(sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>) -> Self {
        Self { sender }
    }

    /// Wait for a USB host to connect and assert DTR on this CDC interface.
    #[allow(dead_code)]
    pub async fn wait_connection(&mut self) {
        self.sender.wait_connection().await;
    }

    /// Wait for a USB host to assert DTR on this CDC interface.
    pub async fn wait_dtr(&mut self) {
        while !self.sender.dtr() {
            embassy_time::Timer::after_millis(10).await;
        }
    }

    /// Write a mandatory proof marker to CDC 1.
    #[cfg(any(feature = "usb-remote", feature = "triple-remote"))]
    pub async fn write_marker(&mut self, msg: &str) -> Result<(), UsbEvidenceError> {
        self.write_all_mandatory(msg.as_bytes()).await?;
        self.write_all_mandatory(b"\n").await
    }
}

pub struct RuntimeTranscriptIdentity {
    boot_id: HString<RUNTIME_ID_BYTES>,
    active_play_id: HString<RUNTIME_ID_BYTES>,
}

impl RuntimeTranscriptIdentity {
    pub fn new(plan_id: &str, host_id: &str) -> Self {
        let mut rng = RoscRng;
        let ticks = Instant::now().as_ticks();
        let entropy_a = rng.next_u64();
        let entropy_b = rng.next_u64();
        let mut boot_id = HString::new();
        let _ = write!(
            boot_id,
            "conduit-pico-w-signal/runtime-boot:{ticks:016x}:{entropy_a:016x}{entropy_b:016x}"
        );
        let digest = conduit_core::active_play_digest(plan_id, host_id, &boot_id, 0);
        let mut active_play_id = HString::new();
        for byte in digest {
            let _ = write!(active_play_id, "{byte:02x}");
        }
        Self {
            boot_id,
            active_play_id,
        }
    }

    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    pub fn active_play_id(&self) -> &str {
        &self.active_play_id
    }
}

#[derive(Clone, Copy)]
pub struct BootIdentity {
    pub firmware_build_id: &'static str,
    pub source_document_id: &'static str,
    pub checked_form_id: &'static str,
    pub expanded_form_id: &'static str,
    pub plan_id: &'static str,
    pub fragment_id: &'static str,
    pub host_id: &'static str,
    pub boot_id: &'static str,
    pub boot_evidence_id: &'static str,
}

#[derive(Clone, Copy)]
pub struct PresentationReceiptIdentity {
    pub firmware_build_id: &'static str,
    pub source_document_id: &'static str,
    pub checked_form_id: &'static str,
    pub expanded_form_id: &'static str,
    pub plan_id: &'static str,
    pub fragment_id: &'static str,
    pub host_id: &'static str,
    pub boot_id: &'static str,
    pub active_play_id: &'static str,
    pub presentation_id: &'static str,
    pub evidence_id: &'static str,
}

#[derive(Clone, Copy)]
pub struct TerminalIdentity {
    pub firmware_build_id: &'static str,
    pub source_document_id: &'static str,
    pub checked_form_id: &'static str,
    pub expanded_form_id: &'static str,
    pub plan_id: &'static str,
    pub fragment_id: &'static str,
    pub host_id: &'static str,
    pub boot_id: &'static str,
    pub active_play_id: &'static str,
    pub evidence_id: &'static str,
}

/// Task that runs the USB device state machine.
#[embassy_executor::task]
pub async fn usb_task_spawn(device: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    let mut device = device;
    device.run().await
}



impl UsbCdc {
    /// Write the boot-scoped identity record for this generated firmware image.
    pub async fn write_boot_identity(
        &mut self,
        identity: BootIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbEvidenceError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit-pico-w-signal/boot@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"evidence_id\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                runtime.boot_id(),
                runtime.active_play_id(),
                identity.boot_evidence_id,
            ),
        )
        .map_err(|_| UsbEvidenceError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a machine-readable receipt for one Signal presentation.
    pub async fn write_receipt(
        &mut self,
        sequence: u64,
        level: bool,
        identity: PresentationReceiptIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbEvidenceError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit-pico-w-signal/receipt@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"sequence\":{},",
                    "\"level\":{},",
                    "\"presentation_id\":\"{}\",",
                    "\"evidence_id\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                runtime.boot_id(),
                runtime.active_play_id(),
                sequence,
                level,
                identity.presentation_id,
                identity.evidence_id,
            ),
        )
        .map_err(|_| UsbEvidenceError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a terminal completion record.
    pub async fn write_terminal(
        &mut self,
        success: bool,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbEvidenceError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit-pico-w-signal/terminal@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"success\":{},",
                    "\"evidence_id\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                runtime.boot_id(),
                runtime.active_play_id(),
                success,
                identity.evidence_id,
            ),
        )
        .map_err(|_| UsbEvidenceError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a kernel error record.
    #[allow(dead_code)]
    pub async fn write_error(
        &mut self,
        e: conduit_kernel::scheduler::SchedulerError,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbEvidenceError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit-pico-w-signal/terminal@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"success\":false,",
                    "\"evidence_id\":\"{}\",",
                    "\"error\":\"{:?}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                runtime.boot_id(),
                runtime.active_play_id(),
                identity.evidence_id,
                e,
            ),
        )
        .map_err(|_| UsbEvidenceError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write terminal failure evidence for a transport/session/kernel failure
    /// that is not representable as a scheduler error value.
    #[cfg(any(feature = "usb-remote", feature = "triple-remote"))]
    pub async fn write_failure(
        &mut self,
        code: &str,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbEvidenceError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit-pico-w-signal/terminal@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"success\":false,",
                    "\"evidence_id\":\"{}\",",
                    "\"error_code\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                runtime.boot_id(),
                runtime.active_play_id(),
                identity.evidence_id,
                code,
            ),
        )
        .map_err(|_| UsbEvidenceError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    async fn write_all_mandatory(&mut self, data: &[u8]) -> Result<(), UsbEvidenceError> {
        let mut offset = 0;
        while offset < data.len() {
            let chunk_len = (data.len() - offset).min(MAX_PACKET_SIZE as usize);
            if self
                .sender
                .write_packet(&data[offset..offset + chunk_len])
                .await
                .is_err()
            {
                return Err(UsbEvidenceError::Disconnected);
            }
            offset += chunk_len;
        }
        Ok(())
    }

}
