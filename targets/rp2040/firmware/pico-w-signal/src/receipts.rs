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
pub(crate) const RECEIPT_BUFFER_BYTES: usize = 2048;
const RUNTIME_ID_BYTES: usize = 128;
// Keep every CDC IN transfer short. If a long record is emitted as consecutive
// maximum-sized packets, the host can wait for the terminating short packet
// while the device endpoint queue fills before it can submit that packet.
const SIGN_WRITE_CHUNK_BYTES: usize = MAX_PACKET_SIZE as usize - 1;

pub struct UsbCdc {
    sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbSignError {
    FormatOverflow,
    Disconnected,
}

impl UsbCdc {
    pub fn new(
        sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
    ) -> Self {
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

    /// Observe whether the current sign consumer still owns this CDC session.
    #[cfg(any(feature = "bluetooth-line", feature = "distributed-lenia"))]
    pub fn dtr(&self) -> bool {
        self.sender.dtr()
    }

    /// Write a mandatory proof marker to CDC 1.
    #[cfg(any(
        feature = "usb-remote",
        feature = "triple-remote",
        feature = "bluetooth-line",
        feature = "distributed-lenia",
        feature = "light-switch"
    ))]
    pub async fn write_marker(&mut self, msg: &str) -> Result<(), UsbSignError> {
        self.write_all_mandatory(msg.as_bytes()).await?;
        self.write_all_mandatory(b"\n").await
    }

    #[cfg(feature = "distributed-lenia")]
    pub async fn write_lenia_boot(
        &mut self,
        runtime: &RuntimeTranscriptIdentity,
        plan_id: &str,
        host_id: &str,
    ) -> Result<(), UsbSignError> {
        let mut record = HString::<RECEIPT_BUFFER_BYTES>::new();
        write!(
            record,
            "CONDUIT_LENIA_BOOT plan={plan_id} host={host_id} boot={}",
            runtime.boot_id()
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_marker(&record).await
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

    /// Seed the network stack from this physical boot's already-recorded
    /// entropy so DHCP transaction identities are not reused across boots.
    #[cfg(feature = "wifi-bootstrap")]
    pub fn network_seed(&self) -> u64 {
        let digest = conduit_core::active_play_digest(
            crate::network_image::PLAN_ID,
            crate::network_image::HOST_ID,
            &self.boot_id,
            1,
        );
        u64::from_le_bytes([
            digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
        ])
    }

    /// Bind a distinct immutable Plan/Play to the same physical boot.
    #[cfg(feature = "wifi-bootstrap")]
    pub fn for_plan(&self, plan_id: &str, host_id: &str) -> Self {
        let boot_id = self.boot_id.clone();
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
    pub boot_sign_id: &'static str,
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
    pub sign_id: &'static str,
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
    pub sign_id: &'static str,
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
    ) -> Result<(), UsbSignError> {
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
                    "\"sign_id\":\"{}\"",
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
                identity.boot_sign_id,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a machine-readable receipt for one Signal presentation.
    pub async fn write_receipt(
        &mut self,
        sequence: u64,
        level: bool,
        identity: PresentationReceiptIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbSignError> {
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
                    "\"sign_id\":\"{}\"",
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
                identity.sign_id,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a terminal completion record.
    pub async fn write_terminal(
        &mut self,
        success: bool,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbSignError> {
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
                    "\"sign_id\":\"{}\"",
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
                identity.sign_id,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write a kernel error record.
    #[allow(dead_code)]
    pub async fn write_error(
        &mut self,
        e: conduit_kernel::scheduler::SchedulerError,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbSignError> {
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
                    "\"sign_id\":\"{}\",",
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
                identity.sign_id,
                e,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    /// Write terminal failure sign for a transport/session/kernel failure
    /// that is not representable as a scheduler error value.
    #[cfg(any(feature = "usb-remote", feature = "triple-remote"))]
    #[cfg(not(feature = "wifi-bootstrap"))]
    pub async fn write_failure(
        &mut self,
        code: &str,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbSignError> {
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
                    "\"sign_id\":\"{}\",",
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
                identity.sign_id,
                code,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub(crate) async fn write_all_mandatory(&mut self, data: &[u8]) -> Result<(), UsbSignError> {
        let mut offset = 0;
        while offset < data.len() {
            let chunk_len = (data.len() - offset).min(SIGN_WRITE_CHUNK_BYTES);
            if self
                .sender
                .write_packet(&data[offset..offset + chunk_len])
                .await
                .is_err()
            {
                return Err(UsbSignError::Disconnected);
            }
            offset += chunk_len;
        }
        Ok(())
    }
}
