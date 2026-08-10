//! One finite physical AP/DHCP/DNS/HTTP Hello appliance composition.

use core::fmt::Write as _;

use embassy_executor::Spawner;
use embassy_futures::select::{select3, Either3};
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Config, IpEndpoint, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4};
use embassy_rp::clocks::RoscRng;
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::Peri;
use embassy_time::{with_timeout, Duration};
use heapless::String as HString;
use static_cell::StaticCell;

use crate::receipts::UsbCdc;

const HOST_ID: &str = "pico/appliance-hello";
const FIRMWARE_BUILD_ID: &str = env!("CONDUIT_PICO_APPLIANCE_BUILD_ID");
const MAXIMUM_HTTP_RESPONSE_BYTES: usize = conduit_net::MAXIMUM_HTTP_RESPONSE_BYTES as usize;
static NETWORK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

#[embassy_executor::task]
async fn appliance_network_task(
    mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>,
) -> ! {
    runner.run().await
}

#[derive(Clone, Copy)]
enum ServiceEvent {
    Dhcp(conduit_net::DhcpResponse),
    Dns,
    Http,
    Failure(conduit_net::ApplianceFailure),
}

#[allow(
    clippy::too_many_arguments,
    reason = "the physical Pico adapter names every fixed radio peripheral and artifact"
)]
pub async fn run(
    spawner: &Spawner,
    sign: &mut UsbCdc,
    pio0: Peri<'static, PIO0>,
    dma: Peri<'static, DMA_CH0>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
    clm: &'static [u8],
) -> ! {
    sign.wait_dtr().await;
    let runtime_boot = runtime_boot_id();
    let (device, mut control) = match crate::radio::init_cyw43_network(
        spawner, pio0, dma, pin23, pin24, pin25, pin29, fw, nvram, clm,
    )
    .await
    {
        Ok(radio) => radio,
        Err(_) => {
            write_sign(
                sign,
                &runtime_boot,
                1,
                "failure",
                Some("radio-initialization-failed"),
                None,
            )
            .await;
            loop {
                core::future::pending::<()>().await;
            }
        }
    };
    if with_timeout(
        Duration::from_secs(10),
        control.start_ap_open(conduit_net::APPLIANCE_SSID, 6),
    )
    .await
    .is_err()
    {
        terminal_failure(sign, &runtime_boot, 1, "ap-base-lost").await;
    }
    let config = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    });
    let seed = network_seed(&runtime_boot);
    let (stack, runner) = embassy_net::new(
        device,
        config,
        NETWORK_RESOURCES.init(StackResources::new()),
        seed,
    );
    spawner.spawn(appliance_network_task(runner).unwrap());

    let mut dhcp_rx_meta = [PacketMetadata::EMPTY; 1];
    let mut dhcp_tx_meta = [PacketMetadata::EMPTY; 1];
    let mut dhcp_rx = [0; conduit_net::MAXIMUM_DHCP_PACKET_BYTES];
    let mut dhcp_tx = [0; conduit_net::MAXIMUM_DHCP_PACKET_BYTES];
    let mut dhcp = UdpSocket::new(
        stack,
        &mut dhcp_rx_meta,
        &mut dhcp_rx,
        &mut dhcp_tx_meta,
        &mut dhcp_tx,
    );
    if dhcp.bind(67).is_err() {
        terminal_failure(sign, &runtime_boot, 1, "dhcp-base-lost").await;
    }

    let mut dns_rx_meta = [PacketMetadata::EMPTY; 1];
    let mut dns_tx_meta = [PacketMetadata::EMPTY; 1];
    let mut dns_rx = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let mut dns_tx = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let mut dns = UdpSocket::new(
        stack,
        &mut dns_rx_meta,
        &mut dns_rx,
        &mut dns_tx_meta,
        &mut dns_tx,
    );
    if dns.bind(53).is_err() {
        terminal_failure(sign, &runtime_boot, 1, "dns-base-lost").await;
    }

    let mut http_rx = [0; conduit_net::MAXIMUM_HTTP_REQUEST_BYTES as usize];
    let mut http_tx = [0; MAXIMUM_HTTP_RESPONSE_BYTES];
    let mut http = TcpSocket::new(stack, &mut http_rx, &mut http_tx);
    let mut leases = conduit_net::DhcpLeasePool::default();
    let mut sequence = 1_u16;
    write_sign(sign, &runtime_boot, sequence, "ap-ready", None, None).await;

    loop {
        let event = match select3(
            serve_dhcp_once(&dhcp, &mut leases),
            serve_dns_once(&dns),
            serve_http_once(&mut http),
        )
        .await
        {
            Either3::First(event) | Either3::Second(event) | Either3::Third(event) => event,
        };
        match event {
            ServiceEvent::Dhcp(response) => {
                sequence += 1;
                match response.kind {
                    conduit_net::DhcpResponseKind::Offer => {
                        write_sign(
                            sign,
                            &runtime_boot,
                            sequence,
                            "client-associated",
                            None,
                            Some(response.lease.address),
                        )
                        .await;
                    }
                    conduit_net::DhcpResponseKind::Acknowledgement => {
                        write_sign(
                            sign,
                            &runtime_boot,
                            sequence,
                            "dhcp-lease",
                            None,
                            Some(response.lease.address),
                        )
                        .await;
                    }
                }
            }
            ServiceEvent::Dns => {
                sequence += 1;
                write_sign(sign, &runtime_boot, sequence, "dns-request", None, None).await;
                sequence += 1;
                write_sign(sign, &runtime_boot, sequence, "dns-response", None, None).await;
            }
            ServiceEvent::Http => {
                sequence += 1;
                write_sign(sign, &runtime_boot, sequence, "http-request", None, None).await;
                sequence += 1;
                write_sign(sign, &runtime_boot, sequence, "http-response", None, None).await;
                sequence += 1;
                write_sign(sign, &runtime_boot, sequence, "terminal", None, None).await;
                core::future::pending::<()>().await;
            }
            ServiceEvent::Failure(failure) => {
                sequence += 1;
                let code = failure_code(failure);
                write_sign(
                    sign,
                    &runtime_boot,
                    sequence,
                    "failure",
                    Some(code),
                    None,
                )
                .await;
                sequence += 1;
                write_sign(
                    sign,
                    &runtime_boot,
                    sequence,
                    "terminal",
                    Some(code),
                    None,
                )
                .await;
                core::future::pending::<()>().await;
            }
        }
    }
}

async fn serve_dhcp_once(
    socket: &UdpSocket<'_>,
    leases: &mut conduit_net::DhcpLeasePool,
) -> ServiceEvent {
    let mut request = [0; conduit_net::MAXIMUM_DHCP_PACKET_BYTES];
    let (len, _) = match socket.recv_from(&mut request).await {
        Ok(value) => value,
        Err(_) => return ServiceEvent::Failure(conduit_net::ApplianceFailure::OversizedDhcpRequest),
    };
    let mut response = [0; conduit_net::MAXIMUM_DHCP_PACKET_BYTES];
    match conduit_net::answer_appliance_dhcp(&request[..len], leases, &mut response) {
        Ok(answer) => {
            let remote = IpEndpoint::new(Ipv4Address::BROADCAST.into(), 68);
            if socket.send_to(&response[..answer.len], remote).await.is_err() {
                ServiceEvent::Failure(conduit_net::ApplianceFailure::ServiceBaseLost(
                    conduit_net::ApplianceService::Dhcp,
                ))
            } else {
                ServiceEvent::Dhcp(answer)
            }
        }
        Err(error) => ServiceEvent::Failure(error),
    }
}

async fn serve_dns_once(socket: &UdpSocket<'_>) -> ServiceEvent {
    let mut request = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let (len, remote) = match socket.recv_from(&mut request).await {
        Ok(value) => value,
        Err(_) => return ServiceEvent::Failure(conduit_net::ApplianceFailure::OversizedDnsRequest),
    };
    let mut response = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    match conduit_net::answer_appliance_dns(&request[..len], &mut response) {
        Ok(response_len) => {
            if socket.send_to(&response[..response_len], remote).await.is_err() {
                ServiceEvent::Failure(conduit_net::ApplianceFailure::ServiceBaseLost(
                    conduit_net::ApplianceService::Dns,
                ))
            } else {
                ServiceEvent::Dns
            }
        }
        Err(error) => ServiceEvent::Failure(error),
    }
}

async fn serve_http_once(socket: &mut TcpSocket<'_>) -> ServiceEvent {
    if socket.accept(80).await.is_err() {
        return ServiceEvent::Failure(conduit_net::ApplianceFailure::ServiceBaseLost(
            conduit_net::ApplianceService::Http,
        ));
    }
    let mut request = [0; conduit_net::MAXIMUM_HTTP_REQUEST_BYTES as usize];
    let mut used = 0;
    while used < request.len() && !request[..used].ends_with(b"\r\n\r\n") {
        match socket.read(&mut request[used..]).await {
            Ok(0) | Err(_) => {
                socket.abort();
                return ServiceEvent::Failure(conduit_net::ApplianceFailure::MalformedHttpRequest);
            }
            Ok(read) => used += read,
        }
    }
    if used == request.len() && !request.ends_with(b"\r\n\r\n") {
        socket.abort();
        return ServiceEvent::Failure(conduit_net::ApplianceFailure::OversizedHttpRequest);
    }
    let mut response = [0; MAXIMUM_HTTP_RESPONSE_BYTES];
    let response_len = match conduit_net::answer_appliance_http(&request[..used], &mut response) {
        Ok(len) => len,
        Err(error) => {
            socket.abort();
            return ServiceEvent::Failure(error);
        }
    };
    let mut written = 0;
    while written < response_len {
        match socket.write(&response[written..response_len]).await {
            Ok(0) | Err(_) => {
                socket.abort();
                return ServiceEvent::Failure(conduit_net::ApplianceFailure::ServiceBaseLost(
                    conduit_net::ApplianceService::Http,
                ));
            }
            Ok(count) => written += count,
        }
    }
    socket.close();
    if socket.flush().await.is_err() {
        return ServiceEvent::Failure(conduit_net::ApplianceFailure::ServiceBaseLost(
            conduit_net::ApplianceService::Http,
        ));
    }
    ServiceEvent::Http
}

async fn terminal_failure(sign: &mut UsbCdc, runtime_boot: &str, sequence: u16, code: &str) -> ! {
    write_sign(sign, runtime_boot, sequence, "failure", Some(code), None).await;
    write_sign(
        sign,
        runtime_boot,
        sequence + 1,
        "terminal",
        Some(code),
        None,
    )
    .await;
    loop {
        core::future::pending::<()>().await;
    }
}

async fn write_sign(
    sign: &mut UsbCdc,
    runtime_boot: &str,
    sequence: u16,
    kind: &str,
    failure: Option<&str>,
    address: Option<[u8; 4]>,
) {
    // The image identity admits 1 KiB per Sign. The longest current
    // association Sign exceeds 512 bytes once exact build and runtime
    // identities are included, so retain that reviewed image-level bound.
    let mut line = HString::<1024>::new();
    let mut sign_id = HString::<192>::new();
    if write!(
        sign_id,
        "pico/appliance/sign:{runtime_boot}:{sequence:02}"
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    if write!(
        line,
        "{{\"schema\":\"conduit.pico-appliance/sign@1\",\"firmware_build_id\":\"{FIRMWARE_BUILD_ID}\",\"profile\":\"{}\",\"host_id\":\"{HOST_ID}\",\"runtime_boot_id\":\"{runtime_boot}\",\"sequence\":{sequence},\"sign_id\":\"{}\",\"kind\":\"{kind}\"",
        conduit_net::PICO_APPLIANCE_PROFILE,
        sign_id.as_str(),
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    if let Some(code) = failure {
        if write!(line, ",\"failure\":\"{code}\"").is_err() {
            core::future::pending::<()>().await;
        }
    }
    if let Some([a, b, c, d]) = address {
        if write!(line, ",\"address\":\"{a}.{b}.{c}.{d}\"").is_err() {
            core::future::pending::<()>().await;
        }
    }
    if line.push_str("}\n").is_err()
        || sign.write_all_mandatory(line.as_bytes()).await.is_err()
    {
        core::future::pending::<()>().await;
    }
}

fn runtime_boot_id() -> HString<96> {
    let mut rng = RoscRng;
    let mut id = HString::new();
    let _ = write!(
        id,
        "pico/appliance/runtime-boot:{:016x}{:016x}",
        rng.next_u64(),
        rng.next_u64()
    );
    id
}

fn network_seed(runtime_boot: &str) -> u64 {
    let digest = conduit_core::active_play_digest(
        conduit_net::PICO_APPLIANCE_PROFILE,
        HOST_ID,
        runtime_boot,
        0,
    );
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

fn failure_code(failure: conduit_net::ApplianceFailure) -> &'static str {
    use conduit_net::{ApplianceFailure as F, ApplianceService as S};
    match failure {
        F::MissingRadioArtifact => "missing-radio-artifact",
        F::RadioInitializationFailed => "radio-initialization-failed",
        F::DhcpPoolExhausted => "dhcp-pool-exhausted",
        F::MalformedDhcpRequest => "malformed-dhcp-request",
        F::OversizedDhcpRequest => "oversized-dhcp-request",
        F::DhcpAddressMismatch => "dhcp-address-mismatch",
        F::DhcpServerMismatch => "dhcp-server-mismatch",
        F::MalformedDnsRequest => "malformed-dns-request",
        F::OversizedDnsRequest => "oversized-dns-request",
        F::MalformedHttpRequest => "malformed-http-request",
        F::OversizedHttpRequest => "oversized-http-request",
        F::ResponseBufferTooSmall => "response-buffer-too-small",
        F::ServiceBaseLost(S::AccessPoint) => "ap-base-lost",
        F::ServiceBaseLost(S::Dhcp) => "dhcp-base-lost",
        F::ServiceBaseLost(S::Dns) => "dns-base-lost",
        F::ServiceBaseLost(S::Http) => "http-base-lost",
        F::SignCapacityExhausted => "sign-capacity-exhausted",
    }
}
