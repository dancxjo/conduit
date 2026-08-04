use self::dhcp::{DhcpGrant, DhcpLeaseState, DhcpRequest};
use self::discovery::{build_dns_reply, build_mdns_announcement};
use crate::{CONDUIT_REVISION, FIRMWARE_IDENTITY, FULL_PLAN_HASH};
use aligned::{Aligned, A4};
use core::fmt::Write as _;
use embedded_io_async::Write as _;
use portable_atomic::{AtomicBool, AtomicU32, Ordering};
use cyw43::State;
use cyw43_pio::{PioSpi, DEFAULT_CLOCK_DIVIDER};
use embassy_executor::{Executor, Spawner};
use embassy_futures::select::{select, Either};
use embassy_net::{
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    Config, HardwareAddress, Ipv4Address, Ipv4Cidr, IpAddress, IpEndpoint, StaticConfigV4, Stack,
    StackResources,
};
use embassy_rp::bind_interrupts;
use embassy_rp::dma;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, PIN_23, PIN_24, PIN_25, PIN_29};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use static_cell::StaticCell;

mod dhcp;
mod discovery;

const AP_IP_OCTETS: [u8; 4] = [192, 168, 4, 1];
const AP_IP: Ipv4Address = Ipv4Address::new(192, 168, 4, 1);
const AP_SSID_PREFIX: &str = "conduit-";
const INSTANCE_ID_BASE: u32 = 36;
const INSTANCE_ID_MODULUS: u32 = INSTANCE_ID_BASE.pow(4);
const AP_CHANNEL: u8 = 6;
const HTTP_PORT: u16 = 80;
const DNS_PORT: u16 = 53;
const MDNS_PORT: u16 = 5353;
const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const HTTP_TASKS: usize = 2;
const HTTP_FLUSH_TIMEOUT_MS: u64 = 250;
const CONDUIT_ADMISSION_TOKEN: &str = "conduit";
const CONDUIT_PROTOCOL_MAJOR: u16 = 1;
const CONDUIT_PROTOCOL_MINOR_MIN: u16 = 0;
const CONDUIT_PROTOCOL_MINOR_MAX: u16 = 0;
const CONDUIT_SUPPORTED_INTERFACE_HTTP: &str = "http";
const SESSION_SLOT_COUNT: usize = 4;
const SESSION_TTL_MS: u32 = 60_000;
const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;
const AP_IP_TEXT: &str = "192.168.4.1";

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

static EXECUTOR: StaticCell<Executor> = StaticCell::new();
static STATE: StaticCell<State> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<10>> = StaticCell::new();
static CONDUIT_LEGACY_ACCEPTED: AtomicBool = AtomicBool::new(false);
static ADMISSION_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static ADMISSION_ACCEPTS: AtomicU32 = AtomicU32::new(0);
static ADMISSION_REJECTS: AtomicU32 = AtomicU32::new(0);
static ADMISSION_REPLAYS: AtomicU32 = AtomicU32::new(0);
static HTTP_REQUESTS: AtomicU32 = AtomicU32::new(0);
static HTTP_RESPONSE_ERRORS: AtomicU32 = AtomicU32::new(0);
static DNS_REQUESTS: AtomicU32 = AtomicU32::new(0);
static DNS_MISSES: AtomicU32 = AtomicU32::new(0);
static DHCP_REQUESTS: AtomicU32 = AtomicU32::new(0);
static DHCP_OFFERS: AtomicU32 = AtomicU32::new(0);
static DHCP_ACKS: AtomicU32 = AtomicU32::new(0);
static DHCP_ACTIVE_LEASES: AtomicU32 = AtomicU32::new(0);
static SESSION_IDENTITY: [AtomicU32; SESSION_SLOT_COUNT] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static SESSION_HASH: [AtomicU32; SESSION_SLOT_COUNT] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static SESSION_GENERATION: [AtomicU32; SESSION_SLOT_COUNT] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static SESSION_EXPIRES_AT: [AtomicU32; SESSION_SLOT_COUNT] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static SESSION_SEQUENCE: AtomicU32 = AtomicU32::new(1);

const FIRMWARE: Aligned<A4, [u8; 4]> = Aligned([0x00, 0x00, 0x00, 0x00]);
const CLM: &[u8] = &[0x00, 0x00, 0x00, 0x00];
const NVRAM: Aligned<A4, [u8; 4]> = Aligned([0x00, 0x00, 0x00, 0x00]);

pub fn run(peripherals: embassy_rp::Peripherals) -> ! {
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        let _ = spawner.spawn(
            wifi_task(
                spawner,
                peripherals.PIO0,
                peripherals.DMA_CH0,
                peripherals.PIN_23,
                peripherals.PIN_24,
                peripherals.PIN_25,
                peripherals.PIN_29,
            )
            .expect("spawn wifi task"),
        );
    });
    loop {}
}

#[embassy_executor::task]
async fn wifi_task(
    spawner: Spawner,
    pio0: Peri<'static, PIO0>,
    dma0: Peri<'static, DMA_CH0>,
    wifi_power: Peri<'static, PIN_23>,
    wifi_dio: Peri<'static, PIN_24>,
    wifi_cs: Peri<'static, PIN_25>,
    wifi_clk: Peri<'static, PIN_29>,
) -> ! {
    if let Some((stack, ap_ssid)) = start_wifi_ap(
        spawner,
        pio0,
        dma0,
        wifi_power,
        wifi_dio,
        wifi_cs,
        wifi_clk,
    )
    .await
    {
        for _ in 0..HTTP_TASKS {
            let _ = spawner.spawn(http_task(stack, ap_ssid.clone()).expect("spawn http task"));
        }
        let _ = spawner.spawn(dns_task(stack).expect("spawn dns task"));
        let _ = spawner.spawn(dhcp_task(stack).expect("spawn dhcp task"));
        let _ = spawner.spawn(mdns_task(stack).expect("spawn mdns task"));
        loop {
            Timer::after_secs(3600).await;
        }
    }

    loop {
        Timer::after_secs(60).await;
    }
}

async fn start_wifi_ap(
    spawner: Spawner,
    pio0: Peri<'static, PIO0>,
    dma0: Peri<'static, DMA_CH0>,
    wifi_power: Peri<'static, PIN_23>,
    wifi_dio: Peri<'static, PIN_24>,
    wifi_cs: Peri<'static, PIN_25>,
    wifi_clk: Peri<'static, PIN_29>,
) -> Option<(Stack<'static>, heapless::String<16>)> {
    let pwr = Output::new(wifi_power, Level::Low);
    let cs = Output::new(wifi_cs, Level::High);
    let mut pio = Pio::new(pio0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        wifi_dio,
        wifi_clk,
        dma::Channel::new(dma0, Irqs),
    );

    let state = STATE.init(State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, &FIRMWARE, &NVRAM).await;
    let _ = spawner.spawn(cyw43_runner_task(runner).expect("spawn cyw43 runner"));

    control.init(CLM).await;
    let _ = control
        .set_power_management(cyw43::PowerManagementMode::None)
        .await;

    let config = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(AP_IP, 24),
        dns_servers: Default::default(),
        gateway: None,
    });
    let (stack, net_runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        0x5eed,
    );
    let _ = spawner.spawn(net_runner_task(net_runner).expect("spawn net runner"));
    let ssid = ap_ssid(stack.hardware_address());
    let _ = stack.join_multicast_group(IpAddress::Ipv4(Ipv4Address::new(224, 0, 0, 251)));

    clear_session_state();
    set_conduit_admission(false);
    let _ = control.start_ap_open(ssid.as_str(), AP_CHANNEL).await;
    Some((stack, ssid))
}

#[embassy_executor::task]
async fn cyw43_runner_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_runner_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn http_task(stack: Stack<'static>, ap_ssid: heapless::String<16>) -> ! {
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 2048];
    let mut request = [0; 1024];
    let mut response = String::<1024>::new();
    let mut method: Option<&str>;
    let mut path: Option<&str>;

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(4)));

        if socket.accept(HTTP_PORT).await.is_err() {
            continue;
        }

        let Ok(n) = read_http_request(&mut socket, &mut request).await else {
            socket.abort();
            continue;
        };

        HTTP_REQUESTS.fetch_add(1, Ordering::Relaxed);
        method = request_method(&request[..n]);
        path = request_path(&request[..n]);
        let now_ms = Instant::now().as_millis() as u32;
        let result = match (method, path) {
            (Some("GET"), Some("/") | Some("/index.html")) => {
                write_response(&mut socket, "text/html; charset=utf-8", index_html().as_bytes())
                    .await
            }
            (Some("GET"), Some("/network.json")) => {
                response.clear();
                if let Some(body) = write_network_json(&mut response, ap_ssid.as_str(), now_ms) {
                    write_response(&mut socket, "application/json", body.as_bytes()).await
                } else {
                    write_plain_status(
                        &mut socket,
                        500,
                        "Internal Server Error",
                        b"status encoding failed",
                    )
                    .await
                }
            }
            (Some("GET"), Some("/dhcp.json")) => {
                write_response(&mut socket, "application/json", b"{\"dhcp\":\"offered\"}\n").await
            }
            (Some("GET"), Some("/status.json")) => {
                response.clear();
                if let Some(body) = write_status_json(&mut response, ap_ssid.as_str()) {
                    write_response(&mut socket, "application/json", body.as_bytes()).await
                } else {
                    write_plain_status(
                        &mut socket,
                        500,
                        "Internal Server Error",
                        b"status encoding failed",
                    )
                    .await
                }
            }
            (Some("GET"), Some("/conduit")) => {
                response.clear();
                if let Some(body) = write_conduit_metadata(
                    &mut response,
                    ap_ssid.as_str(),
                    Instant::now().as_millis() as u32,
                ) {
                    write_response(&mut socket, "application/json", body.as_bytes()).await
                } else {
                    write_plain_status(
                        &mut socket,
                        500,
                        "Internal Server Error",
                        b"status encoding failed",
                    )
                    .await
                }
            }
            (Some("POST"), Some("/conduit") | Some("/handshake")) => {
                if let Some(body) = request_body(&request[..n]) {
                    match json_str(body, "kind") {
                        Some("ping") => {
                            let admitted = has_active_admission();
                            response.clear();
                            if let Some(body) = write_conduit_ping(
                                &mut response,
                                ap_ssid.as_str(),
                                admitted,
                                now_ms,
                            ) {
                                write_response(&mut socket, "application/json", body.as_bytes()).await
                            } else {
                                write_plain_status(
                                    &mut socket,
                                    500,
                                    "Internal Server Error",
                                    b"status encoding failed",
                                )
                                .await
                            }
                        }
                        Some("status") => {
                            response.clear();
                            if let Some(body) = write_status_json(&mut response, ap_ssid.as_str()) {
                                write_response(&mut socket, "application/json", body.as_bytes()).await
                            } else {
                                write_plain_status(
                                    &mut socket,
                                    500,
                                    "Internal Server Error",
                                    b"status encoding failed",
                                )
                                .await
                            }
                        }
                        Some("admit") => {
                            match admit_session(body, now_ms).await {
                                Ok(session) => {
                                    ADMISSION_ACCEPTS.fetch_add(1, Ordering::Relaxed);
                                    if let Some(body) = write_conduit_accept(&mut response, &session) {
                                        write_response(&mut socket, "application/json", body.as_bytes())
                                            .await
                                    } else {
                                        ADMISSION_REJECTS.fetch_add(1, Ordering::Relaxed);
                                        write_plain_status(
                                            &mut socket,
                                            500,
                                            "Internal Server Error",
                                            b"status encoding failed",
                                        )
                                        .await
                                    }
                                }
                                Err(reason) => {
                                    ADMISSION_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    if let Some(response) = write_conduit_reject(&mut response, reason)
                                    {
                                        write_response(
                                            &mut socket,
                                            "application/json",
                                            response.as_bytes(),
                                        )
                                        .await
                                    } else {
                                        write_plain_status(
                                            &mut socket,
                                            409,
                                            "Conflict",
                                            b"{\"kind\":\"reject\",\"reason_code\":\"internal\"}\n",
                                        )
                                        .await
                                    }
                                }
                            }
                        }
                        _ => {
                            match admit_session(body, now_ms).await {
                                Ok(session) => {
                                    ADMISSION_ACCEPTS.fetch_add(1, Ordering::Relaxed);
                                    if let Some(body) = write_conduit_accept(&mut response, &session) {
                                        write_response(
                                            &mut socket,
                                            "application/json",
                                            body.as_bytes(),
                                        )
                                        .await
                                    } else {
                                        ADMISSION_REJECTS.fetch_add(1, Ordering::Relaxed);
                                        write_plain_status(
                                            &mut socket,
                                            500,
                                            "Internal Server Error",
                                            b"status encoding failed",
                                        )
                                        .await
                                    }
                                }
                                Err(reason) => {
                                    ADMISSION_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    if let Some(response) =
                                        write_conduit_reject(&mut response, reason)
                                    {
                                        write_response(
                                            &mut socket,
                                            "application/json",
                                            response.as_bytes(),
                                        )
                                        .await
                                    } else {
                                        write_plain_status(
                                            &mut socket,
                                            400,
                                            "Bad Request",
                                            b"unsupported conduit request",
                                        )
                                        .await
                                    }
                                }
                            }
                        }
                    }
                } else {
                    write_plain_status(&mut socket, 400, "Bad Request", b"request body missing").await
                }
            }
            _ => write_plain_status(&mut socket, 404, "Not Found", b"not found").await,
        };

        match result {
            Ok(true) => socket.close(),
            Ok(false) | Err(_) => {
                HTTP_RESPONSE_ERRORS.fetch_add(1, Ordering::Relaxed);
                socket.abort();
            }
        }
    }
}

#[embassy_executor::task]
async fn dns_task(stack: Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buffer = [0; 512];
    let mut tx_meta = [PacketMetadata::EMPTY; 2];
    let mut tx_buffer = [0; 512];
    let mut request = [0; 512];

    loop {
        let mut socket =
            UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
        if socket.bind(DNS_PORT).is_err() {
            Timer::after_secs(5).await;
            continue;
        }

        loop {
            let Ok((len, endpoint)) = socket.recv_from(&mut request).await else {
                continue;
            };
            DNS_REQUESTS.fetch_add(1, Ordering::Relaxed);
            let mut response = [0; 512];
            let Some(reply) = build_dns_reply(
                &request[..len],
                &mut response,
                AP_IP_OCTETS,
                Instant::now().as_millis() as u32,
            ) else {
                DNS_MISSES.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            let _ = socket.send_to(reply, endpoint).await;
        }
    }
}

#[embassy_executor::task]
async fn dhcp_task(stack: Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buffer = [0; 1024];
    let mut tx_meta = [PacketMetadata::EMPTY; 2];
    let mut tx_buffer = [0; 1024];
    let mut request = [0; 576];
    let mut leases = DhcpLeaseState::new();
    let endpoint = IpEndpoint::new(
        IpAddress::Ipv4(embassy_net::Ipv4Address::new(255, 255, 255, 255)),
        DHCP_CLIENT_PORT,
    );

    loop {
        let mut socket =
            UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
        if socket.bind(DHCP_SERVER_PORT).is_err() {
            Timer::after_secs(5).await;
            continue;
        }

        loop {
            let Ok((len, _meta)) = socket.recv_from(&mut request).await else {
                continue;
            };
            DHCP_REQUESTS.fetch_add(1, Ordering::Relaxed);
            let Some(dhcp_request) = DhcpRequest::parse(&request[..len]) else {
                continue;
            };
            if dhcp::hostname_is_reserved(dhcp_request.client().requested_hostname()) {
                continue;
            }
            let now_ms = Instant::now().as_millis() as u64;
            let Some(grant) = leases.grant(dhcp_request, now_ms) else {
                continue;
            };
            match grant.reply_message_type() {
                2 => {
                    DHCP_OFFERS.fetch_add(1, Ordering::Relaxed);
                }
                5 => {
                    DHCP_ACKS.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
            DHCP_ACTIVE_LEASES.store(leases.active_count(now_ms) as u32, Ordering::Release);
            let mut reply = [0; 576];
            let Some(body) = dhcp_reply(grant, &request[..len], &mut reply) else {
                continue;
            };
            let _ = socket.send_to(body, endpoint).await;
        }
    }
}

#[embassy_executor::task]
async fn mdns_task(stack: Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buffer = [0; 256];
    let mut tx_meta = [PacketMetadata::EMPTY; 2];
    let mut tx_buffer = [0; 768];
    let mut packet = [0; 768];
    let endpoint = IpEndpoint::new(
        IpAddress::Ipv4(embassy_net::Ipv4Address::new(224, 0, 0, 251)),
        MDNS_PORT,
    );

    loop {
        let mut socket =
            UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
        if socket.bind(MDNS_PORT).is_ok() {
            loop {
                let len = build_mdns_announcement(&mut packet);
                let _ = socket.send_to(&packet[..len], endpoint).await;
                Timer::after_secs(5).await;
            }
        }
        Timer::after_secs(5).await;
    }
}

async fn admit_session(body: &str, now_ms: u32) -> Result<ConduitSession, &'static str> {
    ADMISSION_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    if let Some(token) = json_str(body, "token") {
        if token == CONDUIT_ADMISSION_TOKEN {
            set_conduit_admission(true);
            return Ok(ConduitSession {
                session_id: 0,
                generation: 0,
                session_hash: 0,
                negotiated_protocol_minor: CONDUIT_PROTOCOL_MINOR_MIN,
                replayed: false,
            });
        }
        return Err("invalid_token");
    }

    let protocol_major = json_u32(body, "protocol_major").and_then(|value| u16::try_from(value).ok());
    if protocol_major != Some(CONDUIT_PROTOCOL_MAJOR) {
        return Err("unsupported_protocol");
    }
    let protocol_major = CONDUIT_PROTOCOL_MAJOR;

    let protocol_minor = json_u32(body, "protocol_minor")
        .or_else(|| {
            let minimum = json_u32(body, "protocol_minor_min")?;
            let maximum = json_u32(body, "protocol_minor_max")?;
            if (minimum..=maximum).contains(&(CONDUIT_PROTOCOL_MINOR_MIN as u64)) {
                Some(u64::from(CONDUIT_PROTOCOL_MINOR_MIN))
            } else {
                None
            }
        })
        .and_then(|value| u16::try_from(value).ok());
    if protocol_minor.is_none() {
        return Err("unsupported_protocol");
    }
    let protocol_minor = protocol_minor.unwrap_or(0);

    let protocol_minor_min = json_u32(body, "protocol_minor_min")
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(protocol_minor);
    let protocol_minor_max = json_u32(body, "protocol_minor_max")
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(protocol_minor);
    if protocol_minor < CONDUIT_PROTOCOL_MINOR_MIN || protocol_minor > CONDUIT_PROTOCOL_MINOR_MAX {
        return Err("unsupported_protocol");
    }

    let supported_features_contains_session_id = !json_array_present(body, "supported_features")
        || json_array_contains(body, "supported_features", "session_ids") == Some(true);
    let required_features_contains_session_id = !json_array_present(body, "required_features")
        || json_array_contains(body, "required_features", "session_ids") == Some(true);
    if !supported_features_contains_session_id || !required_features_contains_session_id {
        return Err("missing_required_feature");
    }
    let interface = json_str(body, "interface");
    if interface.is_some_and(|value| !interface_is_supported(value)) {
        return Err("unsupported_interface");
    }
    let request_generation = json_u32(body, "generation").and_then(|value| u32::try_from(value).ok());

    let hello = ConduitHello {
        protocol_major,
        protocol_minor,
        protocol_minor_min,
        protocol_minor_max,
        device_id: json_str(body, "device_id").unwrap_or(""),
        boot_id: json_str(body, "boot_id").unwrap_or(""),
        handshake_nonce: json_str(body, "handshake_nonce").unwrap_or(""),
        supported_features_contains_session_id,
        required_features_contains_session_id,
        interface: json_str(body, "interface"),
        request_generation,
    };
    if !hello.is_valid() {
        return Err("invalid_identity");
    }
    let session = record_or_replay_session(hello, now_ms)?;
    if session.replayed {
        ADMISSION_REPLAYS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(session)
}

#[derive(Clone, Copy)]
struct ConduitHello<'a> {
    protocol_major: u16,
    protocol_minor: u16,
    protocol_minor_min: u16,
    protocol_minor_max: u16,
    device_id: &'a str,
    boot_id: &'a str,
    handshake_nonce: &'a str,
    supported_features_contains_session_id: bool,
    required_features_contains_session_id: bool,
    interface: Option<&'a str>,
    request_generation: Option<u32>,
}

impl<'a> ConduitHello<'a> {
    fn is_valid(&self) -> bool {
        self.protocol_major == CONDUIT_PROTOCOL_MAJOR
            && self.protocol_minor >= CONDUIT_PROTOCOL_MINOR_MIN
            && self.protocol_minor <= CONDUIT_PROTOCOL_MINOR_MAX
            && valid_token(self.device_id)
            && valid_token(self.boot_id)
            && valid_token(self.handshake_nonce)
            && valid_protocol_range(self.protocol_minor_min, self.protocol_minor_max)
            && self.supported_features_contains_session_id
            && self.required_features_contains_session_id
            && if let Some(interface) = self.interface {
                interface_is_nonempty(interface)
            } else {
                true
            }
    }
}

#[derive(Clone, Copy)]
struct ConduitSession {
    session_id: u32,
    generation: u32,
    session_hash: u32,
    negotiated_protocol_minor: u16,
    replayed: bool,
}

fn valid_protocol_range(minor_min: u16, minor_max: u16) -> bool {
    minor_min <= minor_max
        && minor_min <= CONDUIT_PROTOCOL_MINOR_MAX
        && minor_max >= CONDUIT_PROTOCOL_MINOR_MIN
}

fn record_or_replay_session(
    hello: ConduitHello,
    now_ms: u32,
) -> Result<ConduitSession, &'static str> {
    let identity = hello_hash(
        hello.protocol_major,
        hello.protocol_minor,
        hello.device_id,
        hello.boot_id,
        hello.handshake_nonce,
    );
    if let Some(existing) = lookup_active_session(identity, now_ms) {
        if let Some(request_generation) = hello.request_generation {
            if request_generation != existing.generation {
                return Err("stale_generation");
            }
        }
        return Ok(existing);
    }

    let Some(slot) = claim_session_slot(now_ms) else {
        return Err("session_limit_reached");
    };
    let generation = SESSION_SEQUENCE.fetch_add(1, Ordering::AcqRel).wrapping_add(0).max(1);
    let session_hash = hello_hash(
        hello.protocol_minor,
        hello.protocol_major,
        hello.handshake_nonce,
        hello.boot_id,
        hello.device_id,
    );

    SESSION_IDENTITY[slot].store(identity, Ordering::Release);
    SESSION_HASH[slot].store(session_hash, Ordering::Release);
    SESSION_GENERATION[slot].store(generation, Ordering::Release);
    SESSION_EXPIRES_AT[slot].store(
        now_ms.saturating_add(SESSION_TTL_MS),
        Ordering::Release,
    );
    Ok(ConduitSession {
        session_id: identity,
        generation,
        session_hash,
        negotiated_protocol_minor: hello.protocol_minor,
        replayed: false,
    })
}

fn claim_session_slot(now_ms: u32) -> Option<usize> {
    clear_expired_sessions(now_ms);
    for slot in 0..SESSION_SLOT_COUNT {
        if SESSION_IDENTITY[slot].load(Ordering::Acquire) == 0 {
            return Some(slot);
        }
    }
    None
}

fn lookup_active_session(
    requested: u32,
    now_ms: u32,
) -> Option<ConduitSession> {
    for slot in 0..SESSION_SLOT_COUNT {
        if SESSION_IDENTITY[slot].load(Ordering::Acquire) != requested {
            continue;
        }
        let expires_at = SESSION_EXPIRES_AT[slot].load(Ordering::Acquire);
        if now_ms > expires_at {
            continue;
        }
        let generation = SESSION_GENERATION[slot].load(Ordering::Acquire);
        return Some(ConduitSession {
            session_id: requested,
            generation,
            session_hash: SESSION_HASH[slot].load(Ordering::Acquire),
            negotiated_protocol_minor: CONDUIT_PROTOCOL_MINOR_MAX,
            replayed: true,
        });
    }
    None
}

fn clear_expired_sessions(now_ms: u32) {
    for slot in 0..SESSION_SLOT_COUNT {
        let identity = SESSION_IDENTITY[slot].load(Ordering::Acquire);
        if identity == 0 {
            continue;
        }
        let expires_at = SESSION_EXPIRES_AT[slot].load(Ordering::Acquire);
        if now_ms > expires_at {
            SESSION_IDENTITY[slot].store(0, Ordering::Release);
            SESSION_HASH[slot].store(0, Ordering::Release);
            SESSION_GENERATION[slot].store(0, Ordering::Release);
            SESSION_EXPIRES_AT[slot].store(0, Ordering::Release);
        }
    }
}

fn clear_session_state() {
    for slot in 0..SESSION_SLOT_COUNT {
        SESSION_IDENTITY[slot].store(0, Ordering::Release);
        SESSION_HASH[slot].store(0, Ordering::Release);
        SESSION_GENERATION[slot].store(0, Ordering::Release);
        SESSION_EXPIRES_AT[slot].store(0, Ordering::Release);
    }
    SESSION_SEQUENCE.store(1, Ordering::Release);
}

fn active_session_count(now_ms: u32) -> u32 {
    let mut count = 0u32;
    for slot in 0..SESSION_SLOT_COUNT {
        if SESSION_IDENTITY[slot].load(Ordering::Acquire) == 0 {
            continue;
        }
        if now_ms <= SESSION_EXPIRES_AT[slot].load(Ordering::Acquire) {
            count += 1;
        }
    }
    count
}

fn has_active_admission() -> bool {
    CONDUIT_LEGACY_ACCEPTED.load(Ordering::Acquire)
        || active_session_count(Instant::now().as_millis() as u32) > 0
}

fn set_conduit_admission(accepted: bool) {
    CONDUIT_LEGACY_ACCEPTED.store(accepted, Ordering::Release);
}

fn write_status_json<'a>(buffer: &'a mut String<1024>, ssid: &str) -> Option<&'a str> {
    let now_ms = Instant::now().as_millis() as u32;
    let admitted = has_active_admission();
    let active_sessions = active_session_count(now_ms);
    buffer.clear();
    write!(
        buffer,
        r#"{{"kind":"status","ssid":"{ssid}","ip":"{AP_IP_TEXT}","conduit_revision":"{CONDUIT_REVISION}","firmware_identity":"{FIRMWARE_IDENTITY}","full_plan_hash":"{FULL_PLAN_HASH}","admitted":{admitted},"active_sessions":{active_sessions},"attempts":{},"accepts":{},"rejects":{},"replays":{},"http_requests":{},"http_response_errors":{},"dns_requests":{},"dns_misses":{},"dhcp_requests":{},"dhcp_offers":{},"dhcp_acks":{},"dhcp_active_leases":{}}}"#,
        ADMISSION_ATTEMPTS.load(Ordering::Acquire),
        ADMISSION_ACCEPTS.load(Ordering::Acquire),
        ADMISSION_REJECTS.load(Ordering::Acquire),
        ADMISSION_REPLAYS.load(Ordering::Acquire),
        HTTP_REQUESTS.load(Ordering::Acquire),
        HTTP_RESPONSE_ERRORS.load(Ordering::Acquire),
        DNS_REQUESTS.load(Ordering::Acquire),
        DNS_MISSES.load(Ordering::Acquire),
        DHCP_REQUESTS.load(Ordering::Acquire),
        DHCP_OFFERS.load(Ordering::Acquire),
        DHCP_ACKS.load(Ordering::Acquire),
        DHCP_ACTIVE_LEASES.load(Ordering::Acquire),
    )
    .ok()?;
    Some(buffer.as_str())
}

fn write_conduit_metadata<'a>(
    buffer: &'a mut String<1024>,
    ssid: &str,
    now_ms: u32,
) -> Option<&'a str> {
    let active_sessions = active_session_count(now_ms);
    buffer.clear();
    write!(
        buffer,
        r#"{{"kind":"conduit","ssid":"{ssid}","ip":"{AP_IP_TEXT}","active_sessions":{active_sessions},"capabilities":{{"http":true,"dns":true,"dhcp":true}},"session_slot_limit":{SESSION_SLOT_COUNT},"protocol_major":{CONDUIT_PROTOCOL_MAJOR},"protocol_minor_min":{CONDUIT_PROTOCOL_MINOR_MIN},"protocol_minor_max":{CONDUIT_PROTOCOL_MINOR_MAX}}}"#
    )
    .ok()?;
    Some(buffer.as_str())
}

fn write_network_json<'a>(
    buffer: &'a mut String<1024>,
    ssid: &str,
    now_ms: u32,
) -> Option<&'a str> {
    let active_sessions = active_session_count(now_ms);
    buffer.clear();
    write!(
        buffer,
        r#"{{"ssid":"{ssid}","ip":"{AP_IP_TEXT}","dhcp":"offered","active_sessions":{active_sessions},"dhcp_active_leases":{}}}"#,
        DHCP_ACTIVE_LEASES.load(Ordering::Acquire),
    )
    .ok()?;
    Some(buffer.as_str())
}

fn write_conduit_ping<'a>(
    buffer: &'a mut String<1024>,
    ssid: &str,
    admitted: bool,
    now_ms: u32,
) -> Option<&'a str> {
    buffer.clear();
    let active_sessions = active_session_count(now_ms);
    write!(
        buffer,
        r#"{{"kind":"pong","ssid":"{ssid}","admitted":{admitted},"active_sessions":{active_sessions}}}"#
    )
    .ok()?;
    Some(buffer.as_str())
}

fn write_conduit_accept<'a>(buffer: &'a mut String<1024>, session: &ConduitSession) -> Option<&'a str> {
    buffer.clear();
    write!(
        buffer,
        "{{\"kind\":\"admit\",\"session_id\":{},\"generation\":{},\"session_hash\":{},\"protocol_minor\":{},\"replayed\":{}}",
        session.session_id,
        session.generation,
        session.session_hash,
        session.negotiated_protocol_minor,
        session.replayed
    )
    .ok()?;
    Some(buffer.as_str())
}

fn interface_is_nonempty(interface: &str) -> bool {
    !interface.trim().is_empty()
}

fn interface_is_supported(interface: &str) -> bool {
    matches!(interface.trim(), CONDUIT_SUPPORTED_INTERFACE_HTTP)
}

fn write_conduit_reject<'a>(buffer: &'a mut String<1024>, reason: &str) -> Option<&'a str> {
    buffer.clear();
    write!(buffer, "{{\"kind\":\"reject\",\"reason_code\":\"{reason}\"}}").ok()?;
    Some(buffer.as_str())
}

fn dhcp_reply<'a>(
    grant: DhcpGrant,
    request: &[u8],
    reply_buffer: &'a mut [u8; 576],
) -> Option<&'a [u8]> {
    discovery::build_dhcp_reply(grant, request, reply_buffer)
}

fn request_path(request: &[u8]) -> Option<&str> {
    let line_end = request.windows(2).position(|w| w == b"\r\n").unwrap_or(request.len());
    let line = core::str::from_utf8(&request[..line_end]).ok()?;
    let mut parts = line.split(' ');
    let _method = parts.next()?;
    parts.next()
}

fn request_method(request: &[u8]) -> Option<&str> {
    let line_end = request.windows(2).position(|w| w == b"\r\n").unwrap_or(request.len());
    let line = core::str::from_utf8(&request[..line_end]).ok()?;
    line.split(' ').next()
}

fn request_body(request: &[u8]) -> Option<&str> {
    let body_start = request.windows(4).position(|w| w == b"\r\n\r\n")?.saturating_add(4);
    core::str::from_utf8(&request[body_start..]).ok()
}

fn json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let bytes = json.as_bytes();
    let mut index = json_key_end(bytes, key)?;
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b':') {
        return None;
    }
    index = skip_whitespace(bytes, index + 1);
    if bytes.get(index) != Some(&b'"') {
        return None;
    }
    let start = index.saturating_add(1);
    index = start;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let end = index;
            return json.get(start..end);
        }
        index += 1;
    }
    None
}

fn json_u32(json: &str, key: &str) -> Option<u64> {
    let bytes = json.as_bytes();
    let mut index = json_key_end(bytes, key)?;
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b':') {
        return None;
    }
    index = skip_whitespace(bytes, index + 1);
    let start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if start == index {
        return None;
    }
    core::str::from_utf8(&bytes[start..index]).ok()?.parse::<u64>().ok()
}

fn json_array_present(json: &str, key: &str) -> bool {
    let bytes = json.as_bytes();
    let Some(mut index) = json_key_end(bytes, key) else {
        return false;
    };
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b':') {
        return false;
    }
    index = skip_whitespace(bytes, index + 1);
    bytes.get(index) == Some(&b'[')
}

fn json_array_contains(json: &str, key: &str, wanted: &str) -> Option<bool> {
    let bytes = json.as_bytes();
    let mut index = json_key_end(bytes, key)?;
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b':') {
        return None;
    }
    index = skip_whitespace(bytes, index + 1);
    if bytes.get(index) != Some(&b'[') {
        return None;
    }
    index += 1;

    loop {
        index = skip_whitespace(bytes, index);
        if index >= bytes.len() {
            return None;
        }
        match bytes.get(index) {
            Some(b']') => return Some(false),
            Some(b'"') => {}
            _ => return None,
        }

        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index] != b'"' {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        if &json[start..index] == wanted {
            return Some(true);
        }
        index = skip_whitespace(bytes, index + 1);
        match bytes.get(index) {
            Some(b',') => index += 1,
            Some(b']') => return Some(false),
            Some(_) => return None,
            None => return None,
        }
    }
}

fn json_key_end(bytes: &[u8], key: &str) -> Option<usize> {
    let needle = key.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut matched = 0usize;
        while j < bytes.len() && matched < needle.len() && bytes[j] == needle[matched] {
            j += 1;
            matched += 1;
        }
        if matched == needle.len() && bytes.get(j) == Some(&b'"') {
            return Some(j + 1);
        }
        i += 1;
    }
    None
}

fn skip_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn hello_hash(
    protocol_major: u16,
    protocol_minor: u16,
    device_id: &str,
    boot_id: &str,
    handshake_nonce: &str,
) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for value in protocol_major.to_le_bytes() {
        hash ^= value as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for value in protocol_minor.to_le_bytes() {
        hash ^= value as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in device_id.bytes().chain(boot_id.bytes()).chain(handshake_nonce.bytes()) {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    if hash == 0 { 1 } else { hash }
}

fn stable_instance_id(address: HardwareAddress) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    if let HardwareAddress::Ethernet(address) = address {
        for byte in address.as_bytes() {
            hash ^= *byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash % INSTANCE_ID_MODULUS
}

fn ap_ssid(address: HardwareAddress) -> heapless::String<16> {
    let mut ssid = heapless::String::<16>::new();
    let _ = ssid.push_str(AP_SSID_PREFIX);
    let mut value = stable_instance_id(address);
    let mut digits = [b'0'; 4];
    for digit in digits.iter_mut().rev() {
        let remainder = (value % INSTANCE_ID_BASE) as u8;
        *digit = if remainder < 10 {
            b'0' + remainder
        } else {
            b'a' + (remainder - 10)
        };
        value /= INSTANCE_ID_BASE;
    }
    for digit in digits {
        let _ = ssid.push(digit as char);
    }
    ssid
}

fn index_html() -> &'static str {
    "<!doctype html><html><body><h1>Conduit Pico W</h1><p>AP online.</p></body></html>\n"
}

async fn read_http_request(
    socket: &mut TcpSocket<'_>,
    buffer: &mut [u8],
) -> Result<usize, embassy_net::tcp::Error> {
    let mut used = 0usize;
    loop {
        if used == buffer.len() {
            return Ok(used);
        }
        let read = socket.read(&mut buffer[used..]).await?;
        if read == 0 {
            return Ok(used);
        }
        used += read;
        let Some(header_end) = buffer[..used]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        else {
            continue;
        };
        let header = core::str::from_utf8(&buffer[..header_end]).unwrap_or("");
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("Content-Length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        if used >= header_end.saturating_add(content_length) {
            return Ok(used);
        }
    }
}

async fn write_response(
    socket: &mut TcpSocket<'_>,
    content_type: &str,
    body: &[u8],
) -> Result<bool, embassy_net::tcp::Error> {
    let mut header = String::<192>::new();
    let _ = header.push_str("HTTP/1.1 200 OK\r\n");
    let _ = write!(
        &mut header,
        "Content-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(header.as_bytes()).await?;
    socket.write_all(body).await?;
    flush_tcp_with_timeout(socket).await
}

async fn write_plain_status(
    socket: &mut TcpSocket<'_>,
    code: u16,
    text: &str,
    body: &[u8],
) -> Result<bool, embassy_net::tcp::Error> {
    let mut header = String::<128>::new();
    let _ = write!(
        &mut header,
        "HTTP/1.1 {code} {text}\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    socket.write_all(header.as_bytes()).await?;
    socket.write_all(body).await?;
    flush_tcp_with_timeout(socket).await
}

async fn flush_tcp_with_timeout(socket: &mut TcpSocket<'_>) -> Result<bool, embassy_net::tcp::Error> {
    match select(socket.flush(), Timer::after_millis(HTTP_FLUSH_TIMEOUT_MS)).await {
        Either::First(result) => result.map(|()| true),
        Either::Second(()) => Ok(false),
    }
}
