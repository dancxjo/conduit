//! Autonomous environmental diagnostic with an opt-in checked Pico W carrier.
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
#[cfg(feature = "credential-store")]
mod credential_store;
#[cfg(not(any(environment_dht11_tm1637, environment_bosch_tm1637)))]
mod environment;
#[cfg(environment_bosch_tm1637)]
#[path = "environment_bme_tm1637.rs"]
mod environment;
#[cfg(environment_dht11_tm1637)]
#[path = "environment_dht.rs"]
mod environment;
#[cfg(all(feature = "wifi", not(feature = "zenoh-evaluation")))]
mod environment_http;
mod environment_show;
mod flash_observation;
#[cfg(all(feature = "credential-store", feature = "zenoh-evaluation"))]
compile_error!("credential-store requires exclusive unstarted CORE1; zenoh-evaluation owns CORE1");
mod heap;
mod usb_identity;
#[cfg(feature = "wifi")]
mod wifi;
#[cfg(feature = "zenoh-evaluation")]
mod zenoh_evaluation;
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
#[cfg(not(feature = "wifi"))]
embassy_rp::bind_interrupts!(struct UsbIrqs{USBCTRL_IRQ=>usb::InterruptHandler<USB>;});
#[cfg(feature = "wifi")]
use wifi::UsbIrqs;
static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static BOS: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();
static USB_SERIAL: StaticCell<[u8; 16]> = StaticCell::new();
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    device.run().await
}
fn cdc(
    driver: usb::Driver<'static, USB>,
    serial: Option<&'static str>,
) -> (UsbDevice<'static, usb::Driver<'static, USB>>, Cdc) {
    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico Environmental USB");
    config.serial_number = serial;
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
    let _foreign_backing = unsafe { heap::initialize() };
    let p = embassy_rp::init(Default::default());
    // Read once before starting either executor/core's hardware tasks. This
    // performs no flash write and creates no Host/Boot or membership identity.
    let serial = {
        let mut flash = embassy_rp::flash::Flash::<
            _,
            _,
            { conduit_rp2040_flash_layout::FLASH_BYTES as usize },
        >::new_blocking(p.FLASH);
        // Read-only physical evidence before any peripheral/core1 tasks start.
        // This does not qualify write timing or create a writable region.
        flash_observation::record(flash.blocking_jedec_id().ok());
        let mut unique = [0; 8];
        let serial = flash
            .blocking_unique_id(&mut unique)
            .ok()
            .and_then(|_| usb_identity::serial(unique))
            .and_then(|bytes| core::str::from_utf8(USB_SERIAL.init(bytes)).ok());
        #[cfg(feature = "credential-store")]
        credential_store::initialize(flash, p.CORE1);
        serial
    };
    config::start_environment!(spawner, p);
    #[cfg(feature = "zenoh-evaluation")]
    zenoh_evaluation::start(p.CORE1, _foreign_backing);
    #[cfg(feature = "wifi")]
    spawner.spawn(
        wifi::run(
            spawner, p.PIO0, p.DMA_CH0, p.DMA_CH1, p.PIN_23, p.PIN_24, p.PIN_25, p.PIN_29,
        )
        .unwrap(),
    );
    let (device, mut class) = cdc(usb::Driver::new(p.USB, UsbIrqs), serial);
    spawner.spawn(usb_task(device).unwrap());
    #[cfg(feature = "wifi")]
    let mut decoder =
        protocol::Decoder::with_join_limit(conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize)
            .expect("shared bounded NetworkJoinRequest")
            .with_selected_peer();
    #[cfg(not(feature = "wifi"))]
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
            #[cfg(all(feature = "wifi", not(feature = "zenoh-evaluation")))]
            protocol::MDNS_QUERY => {
                if let Err(reason) = wifi::mdns(&mut class, id, frame.payload).await {
                    let _ = send(&mut class, protocol::ERROR, id, reason.as_bytes()).await;
                }
            }
            #[cfg(feature = "wifi")]
            protocol::SELECT_PEER => match wifi::select_peer(frame.payload).await {
                Ok(()) => {
                    if let Ok(status) = wifi::status() {
                        let _ =
                            send(&mut class, protocol::INFORMATION, id, status.as_bytes()).await;
                    }
                }
                Err(reason) => {
                    let _ = send(&mut class, protocol::ERROR, id, reason.as_bytes()).await;
                }
            },
            #[cfg(all(feature = "wifi", not(feature = "zenoh-evaluation")))]
            protocol::REQUEST => {
                if let Err(reason) = wifi::http(&mut class, id, frame.payload).await {
                    let _ = send(&mut class, protocol::ERROR, id, reason.as_bytes()).await;
                }
            }
            #[cfg(feature = "wifi")]
            protocol::JOIN => match wifi::configure(frame.payload).await {
                Ok(()) => {
                    if let Ok(status) = wifi::status() {
                        let _ =
                            send(&mut class, protocol::INFORMATION, id, status.as_bytes()).await;
                    }
                }
                Err(reason) => {
                    let _ = send(&mut class, protocol::ERROR, id, reason.as_bytes()).await;
                }
            },
            #[cfg(feature = "wifi")]
            protocol::STATUS => {
                if let Ok(status) = wifi::status() {
                    let _ = send(&mut class, protocol::INFORMATION, id, status.as_bytes()).await;
                }
            }
            #[cfg(not(feature = "wifi"))]
            protocol::STATUS => {
                if let Ok(status) = environment::status() {
                    let _ = send(&mut class, protocol::INFORMATION, id, status.as_bytes()).await;
                }
            }
            protocol::SENSOR_STATUS | protocol::SENSOR_DIAGNOSTIC => {
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
