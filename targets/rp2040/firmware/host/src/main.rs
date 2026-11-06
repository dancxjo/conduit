//! Generic RP2040 Host composition with canonical membership and existing local IO.
//! Local sensor/Face tasks remain diagnostic until ordinary Plot admission exists.
#![no_std]
#![no_main]
extern crate alloc;
use embassy_executor::Spawner;
use embassy_rp::{peripherals::USB, usb};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, UsbDevice,
};
use panic_halt as _;
use static_cell::StaticCell;
mod bootsel;
mod credential_source;
mod environment;
mod environment_http;
mod environment_show;
mod heap;
mod i2c_installation;
mod i2c_owner;
mod membership;
mod native_preparation;
mod radio;
mod resident;
mod resident_preparation;
mod seed;
mod transport;
mod usb_identity;
mod wifi;
#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
}
const BUILD_ID: &str = env!("CONDUIT_RP2040_BUILD_ID");
embassy_rp::bind_interrupts!(struct UsbIrqs{USBCTRL_IRQ=>usb::InterruptHandler<USB>;});
static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static BOS: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();
static SERIAL: StaticCell<[u8; 16]> = StaticCell::new();
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    device.run().await
}
fn usb(
    driver: usb::Driver<'static, USB>,
    serial: Option<&'static str>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    transport::Cdc,
) {
    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("RP2040 Host");
    config.serial_number = serial;
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut builder = Builder::new(
        driver,
        config,
        DEVICE.init([0; 256]),
        CONFIG.init([0; 256]),
        BOS.init([0; 256]),
        CONTROL.init([0; 64]),
    );
    let cdc = CdcAcmClass::new(&mut builder, CDC.init(State::new()), 64);
    (builder.build(), cdc)
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // SAFETY: exclusive boot initialization before spawning allocation users.
    unsafe { heap::initialize() };
    let p = embassy_rp::init(Default::default());
    let (serial, credentials) = {
        let mut flash = credential_source::Device::new_blocking(p.FLASH);
        let mut uid = [0; 8];
        let serial = flash
            .blocking_unique_id(&mut uid)
            .ok()
            .and_then(|_| usb_identity::serial(uid))
            .and_then(|bytes| core::str::from_utf8(SERIAL.init(bytes)).ok());
        let credentials = credential_source::restore(&mut flash);
        (serial, credentials)
    };
    // Independent established local IO keeps running throughout admission.
    config::start_local_io!(spawner, p);
    resident::initialize();
    spawner.spawn(
        wifi::run(
            spawner,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            credentials,
        )
        .unwrap(),
    );
    let (device, class) = usb(usb::Driver::new(p.USB, UsbIrqs), serial);
    spawner.spawn(usb_task(device).unwrap());
    let mut rng = embassy_rp::clocks::RoscRng;
    let mut membership = serial.and_then(|serial| {
        let host = alloc::format!("rp2040/flash/{serial}");
        seed::Seed::load(&host)
            .map(|seed| membership::Membership::new(seed, host, [rng.next_u64(), rng.next_u64()]))
    });
    let mut link = transport::Link::new(class);
    if let Some(member) = membership.as_mut() {
        member.run(&mut link).await;
    }
    loop {
        link.connected().await;
        let result = {
            let mut discard = [0; transport::LIMIT];
            link.receive(&mut discard).await.map(|_| ())
        };
        if result.is_err() {
            link.reset();
        }
        embassy_time::Timer::after_millis(10).await;
    }
}
