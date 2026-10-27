//! Native transport bring-up only. No Body, sensor, display or kernel is claimed.
#![no_std]
#![no_main]
use conduit_pico_usb_http_relay as protocol;
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, Config, Ipv4Address, Stack, StackResources};
use embassy_rp::{peripherals::USB, usb};
use embassy_time::{with_timeout, Duration, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, UsbDevice,
};
mod environment;
mod environment_show;
mod heap;
use panic_halt as _;
use static_cell::StaticCell;
#[path = "../../../firmware/pico-w-signal/src/radio.rs"]
#[allow(unexpected_cfgs, dead_code)] // Only the existing initialized CYW43 network owner is reused.
mod radio;
type Cdc = CdcAcmClass<'static, usb::Driver<'static, USB>>;
static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static BOS: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();
static NETWORK: StaticCell<StackResources<4>> = StaticCell::new();
static FW: aligned::Aligned<aligned::A4, [u8; 231077]> = aligned::Aligned(*include_bytes!(
    "../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static NVRAM:aligned::Aligned<aligned::A4,[u8;742]>=aligned::Aligned(*include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"));
static CLM:&[u8]=include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin");
const _: &[u8]=include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/LICENSE-permissive-binary-license-1.0.txt");
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    device.run().await
}
#[embassy_executor::task]
async fn network_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}
fn cdc(driver: usb::Driver<'static, USB>) -> (UsbDevice<'static, usb::Driver<'static, USB>>, Cdc) {
    let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico W USB HTTP Relay");
    config.serial_number = Some("conduit-pico-usb-http-relay");
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
    let class = CdcAcmClass::new(&mut builder, CDC.init(State::new()), 64);
    (builder.build(), class)
}
async fn packet(class: &mut Cdc, bytes: &[u8]) -> Result<(), &'static str> {
    if !class.dtr() {
        return Err("usb-disconnected");
    }
    with_timeout(Duration::from_secs(1), class.write_packet(bytes))
        .await
        .map_err(|_| "usb-write-timeout")?
        .map_err(|_| "usb-disconnected")
}
async fn send(class: &mut Cdc, kind: u8, id: u32, payload: &[u8]) -> Result<(), &'static str> {
    let header = protocol::header(kind, id, payload.len()).map_err(|_| "response-bound")?;
    packet(class, &header).await?;
    for chunk in payload.chunks(64) {
        packet(class, chunk).await?;
    }
    if !payload.is_empty() && payload.len().is_multiple_of(64) {
        packet(class, &[]).await?;
    }
    Ok(())
}
async fn http(
    class: &mut Cdc,
    stack: Stack<'static>,
    id: u32,
    request: &[u8],
) -> Result<(), &'static str> {
    if !stack.is_link_up() || !stack.is_config_up() {
        return Err("wifi-unavailable");
    }
    let config = stack.config_v4().ok_or("wifi-unconfigured")?;
    if !protocol::peer_network(
        config.address.address().octets(),
        config.address.prefix_len(),
        config.gateway.map(|a| a.octets()),
    ) {
        return Err("wrong-peer-network");
    }
    let mut rx = protocol::Scratch::<4096>::default();
    let mut tx = protocol::Scratch::<2048>::default();
    let mut data = protocol::Scratch::<{ protocol::CHUNK }>::default();
    let mut socket = TcpSocket::new(stack, &mut rx.0, &mut tx.0);
    socket.set_timeout(Some(Duration::from_secs(5)));
    socket
        .connect((Ipv4Address::new(192, 168, 4, 1), 80))
        .await
        .map_err(|_| "tcp-connect")?;
    let mut at = 0;
    while at < request.len() {
        let n = socket
            .write(&request[at..])
            .await
            .map_err(|_| "tcp-write")?;
        if n == 0 {
            return Err("tcp-write-zero");
        }
        at += n;
    }
    let mut total = 0usize;
    loop {
        let n = socket.read(&mut data.0).await.map_err(|_| "tcp-read")?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n)
            .filter(|n| *n <= protocol::MAX_RESPONSE)
            .ok_or("response-capacity")?;
        send(class, protocol::DATA, id, &data.0[..n]).await?;
    }
    if total == 0 {
        return Err("empty-response");
    }
    socket.close();
    send(class, protocol::DONE, id, &(total as u32).to_le_bytes()).await
}
fn peer_ready(stack: Stack<'static>) -> bool {
    stack.config_v4().is_some_and(|config| {
        protocol::peer_network(
            config.address.address().octets(),
            config.address.prefix_len(),
            config.gateway.map(|a| a.octets()),
        )
    })
}
async fn status(class: &mut Cdc, id: u32, stack: Stack<'static>) -> Result<(), &'static str> {
    let mut text = heapless::String::<256>::new();
    let config = stack.config_v4();
    let ip = config
        .as_ref()
        .map(|c| c.address.address().octets())
        .unwrap_or([0; 4]);
    let prefix = config.as_ref().map(|c| c.address.prefix_len()).unwrap_or(0);
    let mut gateway = heapless::String::<20>::new();
    if let Some(address) = config.and_then(|c| c.gateway) {
        let a = address.octets();
        write!(&mut gateway, "[{},{},{},{}]", a[0], a[1], a[2], a[3])
            .map_err(|_| "status-capacity")?;
    } else {
        gateway.push_str("null").map_err(|_| "status-capacity")?;
    }
    write!(&mut text,"{{\"role\":\"{}\",\"link\":{},\"configured\":{},\"peer_ready\":{},\"ipv4\":[{},{},{},{}],\"prefix\":{},\"gateway\":{},\"peer\":\"192.168.4.1:80\",\"request_limit\":{},\"response_limit\":{}}}",protocol::ROLE,stack.is_link_up(),stack.is_config_up(),peer_ready(stack),ip[0],ip[1],ip[2],ip[3],prefix,gateway,protocol::MAX_REQUEST,protocol::MAX_RESPONSE).map_err(|_|"status-capacity")?;
    send(class, protocol::INFORMATION, id, text.as_bytes()).await
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    unsafe {
        heap::initialize();
    }
    let p = embassy_rp::init(Default::default());
    spawner.spawn(environment::run(p.I2C1, p.PIN_7, p.PIN_6, p.PIN_3, p.PIN_4).unwrap());
    let (device, mut class) = cdc(usb::Driver::new(p.USB, radio::UsbIrq));
    spawner.spawn(usb_task(device).unwrap());
    let radio = radio::init_cyw43_network(
        &spawner, p.PIO0, p.DMA_CH0, p.DMA_CH1, p.PIN_23, p.PIN_24, p.PIN_25, p.PIN_29, &FW,
        &NVRAM, CLM,
    )
    .await;
    let (device, mut control) = match radio {
        Ok(pair) => pair,
        Err(_) => loop {
            class.wait_connection().await;
            let _ = send(&mut class, protocol::ERROR, 0, b"radio-initialization").await;
            Timer::after(Duration::from_secs(1)).await;
        },
    };
    let mut rng = embassy_rp::clocks::RoscRng;
    let (stack, runner) = embassy_net::new(
        device,
        Config::dhcpv4(Default::default()),
        NETWORK.init(StackResources::new()),
        rng.next_u64(),
    );
    spawner.spawn(network_task(runner).unwrap());
    // The independent environmental task owns GP3/4 and I2C1 GP6/7.
    let mut decoder = protocol::Decoder::new();
    let mut session = protocol::Session::default();
    let mut input = [0; 64];
    loop {
        class.wait_connection().await;
        while !class.dtr() {
            decoder.reset();
            session.reset();
            Timer::after(Duration::from_millis(25)).await;
        }
        match with_timeout(Duration::from_millis(500), class.read_packet(&mut input)).await {
            Ok(Ok(n)) => {
                if decoder.accept(&input[..n]).is_err() {
                    decoder.reset();
                    let _ = send(&mut class, protocol::ERROR, 0, b"invalid-frame").await;
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
                    let _ = send(&mut class, protocol::ERROR, 0, b"frame-timeout").await;
                }
                continue;
            }
        }
        if !decoder.complete() {
            continue;
        }
        let frame = decoder.frame().unwrap();
        let id = frame.id;
        let outcome = if session.admit(id).is_err() {
            Err("stale-request")
        } else {
            match frame.kind {
                protocol::BOOTSEL_QUERY => {
                    send(
                        &mut class,
                        protocol::BOOTSEL_CHALLENGE,
                        id,
                        protocol::BUILD_ID.as_bytes(),
                    )
                    .await
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
                        Timer::after(Duration::from_millis(100)).await;
                        embassy_rp::rom_data::reset_to_usb_boot(0, 0);
                    }
                    Err("bootsel-ack-failed")
                }
                protocol::SENSOR_DIAGNOSTIC => match environment::diagnostic() {
                    Ok(text) => send(&mut class, protocol::INFORMATION, id, text.as_bytes()).await,
                    Err(e) => Err(e),
                },
                protocol::SENSOR_STATUS => match environment::status() {
                    Ok(text) => send(&mut class, protocol::INFORMATION, id, text.as_bytes()).await,
                    Err(e) => Err(e),
                },
                protocol::STATUS => status(&mut class, id, stack).await,
                protocol::JOIN => match protocol::credentials(frame.payload) {
                    Err(_) => Err("invalid-credentials"),
                    Ok(credentials) => {
                        let options = if credentials.password.is_empty() {
                            cyw43::JoinOptions::new_open()
                        } else {
                            cyw43::JoinOptions::new(credentials.password)
                        };
                        match with_timeout(
                            Duration::from_secs(30),
                            control.join(credentials.ssid, options),
                        )
                        .await
                        {
                            Ok(Ok(())) => {
                                match with_timeout(Duration::from_secs(20), stack.wait_config_up())
                                    .await
                                {
                                    Ok(()) if peer_ready(stack) => {
                                        status(&mut class, id, stack).await
                                    }
                                    Ok(()) => Err("wrong-peer-network"),
                                    Err(_) => Err("dhcp-timeout"),
                                }
                            }
                            _ => Err("association-failed"),
                        }
                    }
                },
                protocol::REQUEST => match with_timeout(
                    Duration::from_secs(30),
                    http(&mut class, stack, id, frame.payload),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err("http-timeout"),
                },
                _ => Err("invalid-command"),
            }
        };
        // Wipe our full command backing, including the borrowed SSID/passphrase,
        // before accepting another command. The radio alone retains association.
        decoder.reset();
        protocol::wipe(&mut input);
        if let Err(reason) = outcome {
            let _ = send(&mut class, protocol::ERROR, id, reason.as_bytes()).await;
        }
    }
}
