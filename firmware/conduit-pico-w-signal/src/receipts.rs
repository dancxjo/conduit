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
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, UsbDevice};
use heapless::String as HString;
use static_cell::StaticCell;

const MAX_PACKET_SIZE: u8 = 64;
const RECEIPT_BUFFER_BYTES: usize = 1536;
const RUNTIME_ID_BYTES: usize = 128;

static USB_STATE: StaticCell<State> = StaticCell::new();
static USB_DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

pub struct UsbCdc {
    sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
}

pub struct RuntimeTranscriptIdentity {
    boot_id: HString<RUNTIME_ID_BYTES>,
    active_play_id: HString<RUNTIME_ID_BYTES>,
}

impl RuntimeTranscriptIdentity {
    pub fn new() -> Self {
        let mut rng = RoscRng;
        let ticks = Instant::now().as_ticks();
        let entropy_a = rng.next_u64();
        let entropy_b = rng.next_u64();
        let mut boot_id = HString::new();
        let _ = write!(
            boot_id,
            "conduit-pico-w-signal/runtime-boot:{ticks:016x}:{entropy_a:016x}{entropy_b:016x}"
        );
        let mut active_play_id = HString::new();
        let _ = write!(active_play_id, "{boot_id}:play:0");
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

impl Default for RuntimeTranscriptIdentity {
    fn default() -> Self {
        Self::new()
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

pub fn init_usb(
    driver: usb::Driver<'static, USB>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    UsbCdc,
) {
    let device_descriptor = USB_DEVICE_DESCRIPTOR.init([0u8; 256]);
    let config_descriptor = USB_CONFIG_DESCRIPTOR.init([0u8; 256]);
    let bos_descriptor = USB_BOS_DESCRIPTOR.init([0u8; 256]);
    let control_buf = USB_CONTROL_BUF.init([0u8; 64]);
    let state = USB_STATE.init(State::new());

    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico W Signal");
    config.serial_number = Some("conduit-pico-w-signal");
    config.max_power = 100;
    config.max_packet_size_0 = MAX_PACKET_SIZE;

    let mut builder = Builder::new(
        driver,
        config,
        device_descriptor,
        config_descriptor,
        bos_descriptor,
        control_buf,
    );

    let class = CdcAcmClass::new(&mut builder, state, MAX_PACKET_SIZE as u16);
    let (sender, _receiver) = class.split();

    let device = builder.build();

    (device, UsbCdc { sender })
}

impl UsbCdc {
    /// Write the boot-scoped identity record for this generated firmware image.
    pub async fn write_boot_identity(
        &mut self,
        identity: BootIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        let _ = core::fmt::write(
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
        );
        self.write_all(line.as_bytes()).await;
    }

    /// Write a machine-readable receipt for one Signal presentation.
    pub async fn write_receipt(
        &mut self,
        sequence: u64,
        level: bool,
        identity: PresentationReceiptIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        let _ = core::fmt::write(
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
        );
        self.write_all(line.as_bytes()).await;
    }

    /// Write a terminal completion record.
    pub async fn write_terminal(
        &mut self,
        success: bool,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        let _ = core::fmt::write(
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
        );
        self.write_all(line.as_bytes()).await;
    }

    /// Write a kernel error record.
    pub async fn write_error(
        &mut self,
        e: conduit_kernel::scheduler::SchedulerError,
        identity: TerminalIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        let _ = core::fmt::write(
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
        );
        self.write_all(line.as_bytes()).await;
    }

    async fn write_all(&mut self, data: &[u8]) {
        let mut offset = 0;
        while offset < data.len() {
            let chunk_len = (data.len() - offset).min(MAX_PACKET_SIZE as usize);
            let _ = self.sender.write_packet(&data[offset..offset + chunk_len]).await;
            offset += chunk_len;
        }
    }
}
