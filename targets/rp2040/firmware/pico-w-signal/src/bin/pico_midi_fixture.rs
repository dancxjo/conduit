//! Independent class-compliant USB-MIDI fixture for the reviewed breadboard.
#![no_std]
#![no_main]

#[path = "../midi_fixture_mapping.rs"]
mod midi_fixture_mapping;

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_time::{Duration, Timer};
use embassy_usb::class::midi::{MidiClass, USB_AUDIO_CLASS};
use embassy_usb::{Builder, Config, UsbDevice};
use midi_fixture_mapping::{FixtureMapping, UsbMidiPacket, MAXIMUM_EVENTS_PER_SCAN};
use panic_halt as _;
use static_cell::StaticCell;

struct NoAllocator;

unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

static DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();

fn usb(
    driver: usb::Driver<'static, USB>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    MidiClass<'static, usb::Driver<'static, USB>>,
) {
    let mut config = Config::new(0x2e8a, 0x000b);
    config.manufacturer = Some("Conduit fixture");
    config.product = Some("Pico W Breadboard MIDI");
    config.serial_number = Some("conduit-pico-w-midi-fixture");
    config.device_class = USB_AUDIO_CLASS;
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut builder = Builder::new(
        driver,
        config,
        DEVICE_DESCRIPTOR.init([0; 256]),
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        CONTROL_BUFFER.init([0; 64]),
    );
    let class = MidiClass::new(&mut builder, 1, 1, 64);
    (builder.build(), class)
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let driver = usb::Driver::new(p.USB, Irqs);
    let (mut device, mut midi) = usb(driver);
    let buttons = [
        Input::new(p.PIN_2, Pull::Up),
        Input::new(p.PIN_3, Pull::Up),
        Input::new(p.PIN_4, Pull::Up),
        Input::new(p.PIN_5, Pull::Up),
        Input::new(p.PIN_6, Pull::Up),
        Input::new(p.PIN_7, Pull::Up),
        Input::new(p.PIN_8, Pull::Up),
        Input::new(p.PIN_9, Pull::Up),
        Input::new(p.PIN_10, Pull::Up),
    ];
    let mut adc = Adc::new_blocking(p.ADC, AdcConfig::default());
    let mut modulation = Channel::new_pin(p.PIN_26, Pull::None);
    let mut expression = Channel::new_pin(p.PIN_27, Pull::None);
    let controls = async {
        let mut mapping = FixtureMapping::new();
        let mut packets = [UsbMidiPacket([0; 4]); MAXIMUM_EVENTS_PER_SCAN];
        midi.wait_connection().await;
        loop {
            let pressed = core::array::from_fn(|index| buttons[index].is_low());
            let samples = [
                adc.blocking_read(&mut modulation).unwrap_or(0),
                adc.blocking_read(&mut expression).unwrap_or(0),
            ];
            let count = mapping.scan(pressed, samples, &mut packets);
            for packet in &packets[..count] {
                if midi.write_packet(&packet.bytes()).await.is_err() {
                    break;
                }
            }
            Timer::after(Duration::from_millis(5)).await;
        }
    };
    join(device.run(), controls).await;
}
