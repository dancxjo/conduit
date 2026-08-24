//! Composite USB device initialisation for dual CDC interfaces.
//!
//! Creates CDC 0 (Conduit UsbCdc link interface) and CDC 1 (sign transcript interface).

use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::{CdcAcmClass, Sender, State};
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;

pub const MAX_PACKET_SIZE: u8 = 64;

#[cfg(feature = "session-control")]
static LINK_STATE: StaticCell<State> = StaticCell::new();
static SIGN_STATE: StaticCell<State> = StaticCell::new();

static USB_DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

#[cfg(feature = "session-control")]
pub struct PicoUsbCdcLine {
    pub class: CdcAcmClass<'static, usb::Driver<'static, USB>>,
}

pub struct UsbSignSender {
    pub sender: Sender<'static, usb::Driver<'static, USB>>,
}

#[cfg(feature = "session-control")]
pub fn init_composite_usb(
    driver: usb::Driver<'static, USB>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    PicoUsbCdcLine,
    UsbSignSender,
) {
    let device_descriptor = USB_DEVICE_DESCRIPTOR.init([0u8; 256]);
    let config_descriptor = USB_CONFIG_DESCRIPTOR.init([0u8; 256]);
    let bos_descriptor = USB_BOS_DESCRIPTOR.init([0u8; 256]);
    let control_buf = USB_CONTROL_BUF.init([0u8; 64]);

    let link_state = LINK_STATE.init(State::new());
    let sign_state = SIGN_STATE.init(State::new());

    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    let (product, serial_number) = if cfg!(feature = "bluetooth-line") {
        ("Pico W Bluetooth Line", "conduit-pico-w-bluetooth-line")
    } else if cfg!(feature = "appliance-hil-client") {
        (
            "Pico W Appliance HIL Client",
            "conduit-pico-hil-client",
        )
    } else {
        ("Pico W Signal", "conduit-pico-w-signal")
    };
    config.product = Some(product);
    config.serial_number = Some(serial_number);
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

    // CDC 0: Link interface (unsplit CdcAcmClass)
    let link_class = CdcAcmClass::new(&mut builder, link_state, MAX_PACKET_SIZE as u16);

    // CDC 1: Sign interface
    let sign_class = CdcAcmClass::new(&mut builder, sign_state, MAX_PACKET_SIZE as u16);
    let (sign_sender, _sign_receiver) = sign_class.split();

    let device = builder.build();

    (
        device,
        PicoUsbCdcLine { class: link_class },
        UsbSignSender {
            sender: sign_sender,
        },
    )
}

#[cfg(not(feature = "session-control"))]
pub fn init_sign_usb(
    driver: usb::Driver<'static, USB>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    UsbSignSender,
) {
    let device_descriptor = USB_DEVICE_DESCRIPTOR.init([0u8; 256]);
    let config_descriptor = USB_CONFIG_DESCRIPTOR.init([0u8; 256]);
    let bos_descriptor = USB_BOS_DESCRIPTOR.init([0u8; 256]);
    let control_buf = USB_CONTROL_BUF.init([0u8; 64]);
    let sign_state = SIGN_STATE.init(State::new());

    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico W Signal Minimal");
    config.serial_number = Some("conduit-pico-w-signal-minimal");
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
    let sign_class = CdcAcmClass::new(&mut builder, sign_state, MAX_PACKET_SIZE as u16);
    let (sign_sender, _sign_receiver) = sign_class.split();
    (
        builder.build(),
        UsbSignSender {
            sender: sign_sender,
        },
    )
}
