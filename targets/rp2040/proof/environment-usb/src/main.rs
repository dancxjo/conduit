//! USB-only autonomous environmental diagnostic; old radio relay is separate.
#![no_std]
#![no_main]
use embassy_executor::Spawner;
use embassy_rp::{peripherals::USB, usb};
use embassy_time::{with_timeout, Duration, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, UsbDevice,
};
use panic_halt as _;
use static_cell::StaticCell;
mod environment;
mod environment_show;
mod heap;
#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
}
// Same bounded transport decoder and exact-build BOOTSEL mechanism as the
// working relay; no copy or edit of that firmware is required.
#[path = "../../usb-http-relay/src/lib.rs"]
#[allow(dead_code, unused_attributes)]
mod protocol;
type Cdc = CdcAcmClass<'static, usb::Driver<'static, USB>>;
embassy_rp::bind_interrupts!(struct UsbIrqs{USBCTRL_IRQ=>usb::InterruptHandler<USB>;});
static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static BOS: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    device.run().await
}
fn cdc(driver: usb::Driver<'static, USB>) -> (UsbDevice<'static, usb::Driver<'static, USB>>, Cdc) {
    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico Environmental USB");
    config.serial_number = Some("conduit-environment-usb");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut b = Builder::new(
        driver,
        config,
        DEVICE.init([0; 256]),
        CONFIG.init([0; 256]),
        BOS.init([0; 256]),
        CONTROL.init([0; 64]),
    );
    let class = CdcAcmClass::new(&mut b, CDC.init(State::new()), 64);
    (b.build(), class)
}
async fn send(c: &mut Cdc, kind: u8, id: u32, payload: &[u8]) -> Result<(), ()> {
    let h = protocol::header(kind, id, payload.len()).map_err(|_| ())?;
    for data in core::iter::once(h.as_slice()).chain(payload.chunks(64)) {
        with_timeout(Duration::from_secs(1), c.write_packet(data))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
    }
    if !payload.is_empty() && payload.len().is_multiple_of(64) {
        with_timeout(Duration::from_secs(1), c.write_packet(&[]))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
    }
    Ok(())
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    unsafe {
        heap::initialize();
    }
    let p = embassy_rp::init(Default::default());
    config::start_environment!(spawner, p);
    let (device, mut class) = cdc(usb::Driver::new(p.USB, UsbIrqs));
    spawner.spawn(usb_task(device).unwrap());
    let mut decoder = protocol::Decoder::new();
    let mut session = protocol::Session::default();
    let mut input = [0u8; 64];
    loop {
        class.wait_connection().await;
        if !class.dtr() {
            decoder.reset();
            session.reset();
            Timer::after_millis(25).await;
            continue;
        }
        match with_timeout(Duration::from_millis(500), class.read_packet(&mut input)).await {
            Ok(Ok(n)) => {
                if decoder.accept(&input[..n]).is_err() {
                    decoder.reset();
                    continue;
                }
            }
            Ok(Err(_)) => {
                decoder.reset();
                session.reset();
                continue;
            }
            Err(_) => {
                if decoder.has_partial() {
                    decoder.reset();
                }
                continue;
            }
        }
        if !decoder.complete() {
            continue;
        }
        let frame = decoder.frame().unwrap();
        let id = frame.id;
        if session.admit(id).is_err() {
            let _ = send(&mut class, protocol::ERROR, id, b"stale-request").await;
            decoder.reset();
            continue;
        }
        match frame.kind {
            protocol::BOOTSEL_QUERY => {
                let _ = send(
                    &mut class,
                    protocol::BOOTSEL_CHALLENGE,
                    id,
                    protocol::BUILD_ID.as_bytes(),
                )
                .await;
            }
            protocol::BOOTSEL_REQUEST if protocol::bootsel_matches(frame.payload) => {
                if send(
                    &mut class,
                    protocol::BOOTSEL_ACK,
                    id,
                    protocol::BUILD_ID.as_bytes(),
                )
                .await
                .is_ok()
                {
                    decoder.reset();
                    protocol::wipe(&mut input);
                    Timer::after_millis(100).await;
                    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
                }
            }
            protocol::STATUS | protocol::SENSOR_STATUS | protocol::SENSOR_DIAGNOSTIC => {
                if let Ok(status) = environment::status() {
                    let _ = send(&mut class, protocol::INFORMATION, id, status.as_bytes()).await;
                }
            }
            _ => {
                let _ = send(&mut class, protocol::ERROR, id, b"unsupported-command").await;
            }
        }
        decoder.reset();
        protocol::wipe(&mut input);
    }
}
