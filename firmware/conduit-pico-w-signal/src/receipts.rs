//! USB CDC receipt emission for machine-readable Pico W signal proof.
//!
//! Emits newline-delimited JSON receipt records over a USB serial port.
//! The verifier (xtask pico verify) reads these records to confirm the
//! exact Signal sequence, levels, and terminal disposition.

use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, UsbDevice};
use heapless::String as HString;
use static_cell::StaticCell;

const MAX_PACKET_SIZE: u8 = 64;

static USB_STATE: StaticCell<State> = StaticCell::new();
static USB_DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

pub struct UsbCdc {
    sender: embassy_usb::class::cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
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
    /// Write a machine-readable receipt for one Signal presentation.
    pub async fn write_receipt(&mut self, sequence: u64, level: bool) {
        let mut line: HString<256> = HString::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!(
                "{{\"schema\":\"conduit-pico-w-signal/receipt@1\",\"sequence\":{},\"level\":{}}}\n",
                sequence,
                level,
            ),
        );
        self.write_all(line.as_bytes()).await;
    }

    /// Write a terminal completion record.
    pub async fn write_terminal(&mut self, success: bool) {
        let mut line: HString<128> = HString::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!(
                "{{\"schema\":\"conduit-pico-w-signal/terminal@1\",\"success\":{}}}\n",
                success,
            ),
        );
        self.write_all(line.as_bytes()).await;
    }

    /// Write a kernel error record.
    pub async fn write_error(&mut self, e: conduit_kernel::scheduler::SchedulerError) {
        let mut line: HString<128> = HString::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!(
                "{{\"schema\":\"conduit-pico-w-signal/terminal@1\",\"success\":false,\"error\":\"{:?}\"}}\n",
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
