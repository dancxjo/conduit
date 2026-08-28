//! Bounded second-Pico client fixture for the physical Hello appliance proof.

use core::fmt::Write as _;

use embassy_executor::Spawner;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, IpAddress, IpEndpoint, Ipv4Address, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::Peri;
use embassy_time::{with_timeout, Duration};
use heapless::String as HString;
use static_cell::StaticCell;

use crate::receipts::UsbCdc;

const HOST_ID: &str = "pico/appliance-hil-client";
const FIRMWARE_BUILD_ID: &str = env!("CONDUIT_PICO_APPLIANCE_BUILD_ID");
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
static NETWORK_RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();

#[derive(Clone, Copy)]
enum ProbeFailure {
    RadioInitialization,
    Association,
    DhcpConfiguration,
    DhcpAddress,
    DnsQuery,
    DnsAddress,
    HttpConnect,
    HttpWrite,
    HttpRead,
    HttpResponse,
}

impl ProbeFailure {
    const fn code(self) -> &'static str {
        match self {
            Self::RadioInitialization => "radio-initialization-failed",
            Self::Association => "ap-association-failed",
            Self::DhcpConfiguration => "dhcp-configuration-failed",
            Self::DhcpAddress => "dhcp-address-mismatch",
            Self::DnsQuery => "dns-query-failed",
            Self::DnsAddress => "dns-address-mismatch",
            Self::HttpConnect => "http-connect-failed",
            Self::HttpWrite => "http-write-failed",
            Self::HttpRead => "http-read-failed",
            Self::HttpResponse => "http-response-mismatch",
        }
    }
}

#[embassy_executor::task]
async fn network_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[allow(
    clippy::too_many_arguments,
    reason = "the physical Pico fixture names every fixed radio peripheral and artifact"
)]
pub async fn run(
    spawner: &Spawner,
    sign: &mut UsbCdc,
    pio0: Peri<'static, PIO0>,
    dma_tx: Peri<'static, DMA_CH0>,
    dma_rx: Peri<'static, DMA_CH1>,
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
        spawner, pio0, dma_tx, dma_rx, pin23, pin24, pin25, pin29, fw, nvram, clm,
    )
    .await
    {
        Ok(radio) => radio,
        Err(_) => terminal_failure(sign, &runtime_boot, ProbeFailure::RadioInitialization).await,
    };

    let (stack, runner) = embassy_net::new(
        device,
        Config::dhcpv4(Default::default()),
        NETWORK_RESOURCES.init(StackResources::new()),
        network_seed(&runtime_boot),
    );
    spawner.spawn(network_task(runner).unwrap());
    match with_timeout(
        PROBE_TIMEOUT,
        control.join(conduit_rp2040_network_realization::APPLIANCE_SSID, cyw43::JoinOptions::new_open()),
    )
    .await
    {
        Ok(Ok(())) => {}
        _ => terminal_failure(sign, &runtime_boot, ProbeFailure::Association).await,
    }
    if with_timeout(PROBE_TIMEOUT, stack.wait_config_up())
        .await
        .is_err()
    {
        terminal_failure(sign, &runtime_boot, ProbeFailure::DhcpConfiguration).await;
    }
    let Some(config) = stack.config_v4() else {
        terminal_failure(sign, &runtime_boot, ProbeFailure::DhcpConfiguration).await
    };
    let leased_address = config.address.address().octets();
    if leased_address[..3] != [192, 168, 4] || !(2..=5).contains(&leased_address[3]) {
        terminal_failure(sign, &runtime_boot, ProbeFailure::DhcpAddress).await;
    }

    let answers = match with_timeout(
        PROBE_TIMEOUT,
        stack.dns_query(conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME, DnsQueryType::A),
    )
    .await
    {
        Ok(Ok(answers)) => answers,
        _ => terminal_failure(sign, &runtime_boot, ProbeFailure::DnsQuery).await,
    };
    let dns_address = answers.first().and_then(|address| match address {
        IpAddress::Ipv4(address) => Some(address.octets()),
    });
    if dns_address != Some(conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS) {
        terminal_failure(sign, &runtime_boot, ProbeFailure::DnsAddress).await;
    }

    let mut tcp_rx = [0_u8; conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES as usize];
    let mut tcp_tx = [0_u8; conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES as usize];
    let mut socket = TcpSocket::new(stack, &mut tcp_rx, &mut tcp_tx);
    socket.set_timeout(Some(Duration::from_secs(5)));
    let endpoint = IpEndpoint::new(Ipv4Address::new(192, 168, 4, 1).into(), 80);
    if socket.connect(endpoint).await.is_err() {
        terminal_failure(sign, &runtime_boot, ProbeFailure::HttpConnect).await;
    }
    let request = b"GET / HTTP/1.1\r\nHost: hello.conduit\r\nConnection: close\r\n\r\n";
    let mut written = 0;
    while written < request.len() {
        let result = socket
            .write_with(|buffer| {
                let count = buffer.len().min(request.len() - written);
                buffer[..count].copy_from_slice(&request[written..written + count]);
                (count, count)
            })
            .await;
        match result {
            Ok(0) | Err(_) => terminal_failure(sign, &runtime_boot, ProbeFailure::HttpWrite).await,
            Ok(count) => written += count,
        }
    }
    if socket.flush().await.is_err() {
        terminal_failure(sign, &runtime_boot, ProbeFailure::HttpWrite).await;
    }
    let mut response = [0_u8; conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES as usize];
    let mut received = 0;
    loop {
        if received == response.len() {
            break;
        }
        match socket.read(&mut response[received..]).await {
            Ok(0) => break,
            Ok(count) => received += count,
            Err(_) => terminal_failure(sign, &runtime_boot, ProbeFailure::HttpRead).await,
        }
    }
    if response[..received] != conduit_rp2040_network_realization::HTTP_HELLO_RESPONSE[..] {
        terminal_failure(sign, &runtime_boot, ProbeFailure::HttpResponse).await;
    }

    write_receipt(sign, &runtime_boot, leased_address, None).await;
    loop {
        core::future::pending::<()>().await;
    }
}

async fn terminal_failure(sign: &mut UsbCdc, runtime_boot: &str, failure: ProbeFailure) -> ! {
    write_receipt(sign, runtime_boot, [0, 0, 0, 0], Some(failure.code())).await;
    loop {
        core::future::pending::<()>().await;
    }
}

async fn write_receipt(
    sign: &mut UsbCdc,
    runtime_boot: &str,
    leased_address: [u8; 4],
    failure: Option<&str>,
) {
    let [a, b, c, d] = leased_address;
    let mut line = HString::<768>::new();
    if write!(
        line,
        "{{\"schema\":\"conduit.pico-appliance/hil-client@1\",\"firmware_build_id\":\"{FIRMWARE_BUILD_ID}\",\"host_id\":\"{HOST_ID}\",\"runtime_boot_id\":\"{runtime_boot}\",\"ssid\":\"{}\",\"terminal\":true,\"success\":{}",
        conduit_rp2040_network_realization::APPLIANCE_SSID,
        failure.is_none(),
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    if let Some(code) = failure {
        if write!(line, ",\"failure\":\"{code}\"").is_err() {
            core::future::pending::<()>().await;
        }
    } else if write!(
        line,
        ",\"leased_address\":\"{a}.{b}.{c}.{d}\",\"dns_name\":\"{}\",\"dns_address\":\"192.168.4.1\",\"http_body\":\"Hello from Conduit\\n\"",
        conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME,
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    if line.push_str("}\n").is_err() || sign.write_all_mandatory(line.as_bytes()).await.is_err() {
        core::future::pending::<()>().await;
    }
}

fn runtime_boot_id() -> HString<96> {
    let mut rng = RoscRng;
    let mut id = HString::new();
    let _ = write!(
        id,
        "pico/appliance-hil-client/runtime-boot:{:016x}{:016x}",
        rng.next_u64(),
        rng.next_u64()
    );
    id
}

fn network_seed(runtime_boot: &str) -> u64 {
    let digest = conduit_core::active_play_digest(
        conduit_rp2040_network_realization::PICO_APPLIANCE_PROFILE,
        HOST_ID,
        runtime_boot,
        0,
    );
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}
