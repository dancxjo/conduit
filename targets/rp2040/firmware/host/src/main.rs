//! Generic RP2040 Host composition with canonical membership and existing local IO.
//! Physical equipment custody and membership are separate from admitted Play.
#![no_std]
#![no_main]
extern crate alloc;
use embassy_executor::Spawner;
use embassy_rp::{peripherals::USB, usb};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, UsbDevice,
};
mod panic_capture;
mod stack_probe;
use static_cell::StaticCell;
mod bootsel;
mod credential_source;
#[cfg(all(not(display_tm1637), not(sensor_dht11)))]
mod environment;
#[cfg(all(display_tm1637, not(sensor_dht11)))]
#[path = "environment_tm1637.rs"]
mod environment;
#[cfg(sensor_dht11)]
#[path = "environment_dht11.rs"]
mod environment;
#[cfg(not(shared_source_execution))]
mod environment_http;
mod environment_show;
#[cfg(sensor_dht11)]
mod equipment;
mod execution_selection;
mod execution_supervisor;
mod heap;
mod i2c_installation;
#[allow(dead_code)] // Supporting native issuance inspection remains independent of execution.
mod i2c_issuance;
mod i2c_owner;
#[cfg(sensor_dht11)]
#[allow(dead_code)] // Cold continuation is separately opt-in and Root-verified.
mod installed_continuation;
mod membership;
#[allow(dead_code)] // Additional bounded composition APIs remain available for qualification.
mod mixed_preparation;
#[allow(dead_code)] // Typed ownership handoff remains available to Root callers.
mod mixed_supervision;
#[allow(dead_code)] // Preparation inspection is separate from execution.
mod mixed_workset;
mod native_preparation;
mod native_publication;
#[cfg(sensor_dht11)]
mod play;
#[cfg(sensor_dht11)]
mod play_run;
#[cfg(sensor_dht11)]
mod play_service;
#[cfg(sensor_dht11)]
mod pulse_owner;
mod radio;
#[cfg(sensor_dht11)]
mod renderer;
mod resident;
mod resident_preparation;
mod retired_resources;
#[cfg(sensor_dht11)]
mod runtime;
mod seed;
#[cfg(sensor_dht11)]
mod tm1637_owner;
#[cfg(display_tm1637)]
mod tm1637_pins;
mod transport;
mod usb_identity;
mod wifi;
#[cfg(sensor_dht11)]
mod workset;
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
#[inline(never)]
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
struct Boot {
    serial: Option<&'static str>,
    nonce: [u64; 2],
    link: transport::Link,
    #[cfg(sensor_dht11)]
    flash: credential_source::Device,
}
// Cold peripheral/USB/Wi-Fi construction must not reserve scratch in every poll.
#[inline(never)]
fn initialize(spawner: Spawner) -> Boot {
    // SAFETY: exclusive boot initialization before spawning allocation users.
    unsafe { heap::initialize() };
    stack_probe::initialize();
    stack_probe::checkpoint(19);
    let p = embassy_rp::init(Default::default());
    let (serial, credentials, flash) = {
        let mut flash = credential_source::Device::new_blocking(p.FLASH);
        let mut uid = [0; 8];
        let serial = flash
            .blocking_unique_id(&mut uid)
            .ok()
            .and_then(|_| usb_identity::serial(uid))
            .and_then(|bytes| core::str::from_utf8(SERIAL.init(bytes)).ok());
        let credentials = credential_source::restore(&mut flash);
        (serial, credentials, flash)
    };
    #[cfg(not(sensor_dht11))]
    drop(flash);
    // Initialize checked equipment. The GPIO profile performs no measurement
    // or display effects here; its Source must first pass complete admission.
    config::start_local_io!(spawner, p);
    stack_probe::checkpoint(20);
    resident::initialize();
    stack_probe::checkpoint(21);
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
    Boot {
        serial,
        nonce: [rng.next_u64(), rng.next_u64()],
        link: transport::Link::new(class),
        #[cfg(sensor_dht11)]
        flash,
    }
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let boot = initialize(spawner);
    let serial = boot.serial;
    let nonce = boot.nonce;
    let mut link = boot.link;
    #[cfg(sensor_dht11)]
    let flash = boot.flash;
    #[cfg(not(sensor_dht11))]
    let mut membership = serial.and_then(|serial| {
        let host = alloc::format!("rp2040/flash/{serial}");
        seed::Seed::load(&host).map(|seed| membership::Membership::new(seed, host, nonce))
    });
    #[cfg(sensor_dht11)]
    if let Some(serial) = serial {
        let host = alloc::format!("rp2040/flash/{serial}");
        if let Some(seed) = seed::Seed::load(&host) {
            let future = core::pin::pin!(runtime::run(seed, host, nonce, &mut link, flash,));
            conduit_mcu_host_runtime::poll_boundary::separate(future).await;
        }
    }
    #[cfg(not(sensor_dht11))]
    if let Some(member) = membership.as_mut() {
        member.run(&mut link).await;
    }
    let future = core::pin::pin!(unconfigured_link(&mut link));
    conduit_mcu_host_runtime::poll_boundary::separate(future).await
}
// This fallback's discard buffer belongs only to the unconfigured carrier poll,
// never to the normal Root/Play call chain.
async fn unconfigured_link(link: &mut transport::Link) -> ! {
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
