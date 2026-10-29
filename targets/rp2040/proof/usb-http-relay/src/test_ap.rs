//! Explicit bench transport mode, independent of measurement and Body custody.
//! Reuse the existing bounded DHCP server; never serve DHCP in relay mode.
use conduit_rp2040_network_realization::{answer_appliance_dhcp, DhcpLeasePool};
use core::cell::Cell;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    ConfigV4, IpEndpoint, Ipv4Address, Ipv4Cidr, Stack, StaticConfigV4,
};
use embassy_time::{with_timeout, Duration, Timer};
use portable_atomic::{AtomicBool, AtomicU32, Ordering};

pub const SSID: &str = "Conduit-Relay-Test";
/// Public, isolated bench credential; never a user network credential.
pub const PASSPHRASE: &str = "conduit-bench-test";
pub fn selected_peer() -> Option<[u8; 6]> {
    (active() && serving()).then(|| critical_section::with(|cs| PEER.borrow(cs).get()))
}
static AP_CUSTODY: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);
static SERVING: AtomicBool = AtomicBool::new(false);
static ACKNOWLEDGED: AtomicU32 = AtomicU32::new(0);
static PEER: critical_section::Mutex<Cell<[u8; 6]>> =
    critical_section::Mutex::new(Cell::new([0; 6]));

pub fn active() -> bool {
    AP_CUSTODY.load(Ordering::Acquire)
}
pub fn serving() -> bool {
    SERVING.load(Ordering::Acquire)
}
pub fn acknowledged() -> u32 {
    ACKNOWLEDGED.load(Ordering::Acquire)
}
pub fn peer() -> [u8; 4] {
    if active() {
        [192, 168, 4, 2]
    } else {
        [192, 168, 4, 1]
    }
}

pub async fn start(
    control: &mut cyw43::Control<'_>,
    stack: Stack<'static>,
    peer: [u8; 6],
) -> Result<(), &'static str> {
    if active() {
        return if ENABLED.load(Ordering::Acquire)
            && serving()
            && critical_section::with(|cs| PEER.borrow(cs).get()) == peer
        {
            Ok(())
        } else {
            Err("test-ap-not-serving")
        };
    }
    if serving() {
        return Err("test-ap-stop-pending");
    }
    with_timeout(Duration::from_secs(10), control.leave())
        .await
        .map_err(|_| "test-ap-leave-timeout")?;
    stack.set_config_v4(ConfigV4::Static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    }));
    // The driver acknowledges its AP configuration, while the independent task
    // below must separately acknowledge its real bound DHCP socket.
    // An interrupted radio command retains AP custody for explicit stop/retry;
    // it must not silently resume upstream DHCP after an ambiguous transition.
    AP_CUSTODY.store(true, Ordering::Release);
    with_timeout(
        Duration::from_secs(10),
        control.start_ap_wpa2(SSID, PASSPHRASE, 6),
    )
    .await
    .map_err(|_| "test-ap-radio-timeout")?;
    critical_section::with(|cs| PEER.borrow(cs).set(peer));
    ENABLED.store(true, Ordering::Release);
    with_timeout(Duration::from_secs(2), async {
        while !serving() {
            Timer::after_millis(10).await;
        }
    })
    .await
    .map_err(|_| "test-ap-dhcp-timeout")
}

pub async fn stop(
    control: &mut cyw43::Control<'_>,
    stack: Stack<'static>,
) -> Result<(), &'static str> {
    if !active() {
        return Ok(());
    }
    ENABLED.store(false, Ordering::Release);
    // Retire the actual listening socket before enabling an upstream DHCP client.
    with_timeout(Duration::from_secs(2), async {
        while serving() {
            Timer::after_millis(10).await;
        }
    })
    .await
    .map_err(|_| "test-ap-stop-timeout")?;
    with_timeout(Duration::from_secs(10), control.close_ap())
        .await
        .map_err(|_| "test-ap-radio-stop-timeout")?;
    AP_CUSTODY.store(false, Ordering::Release);
    stack.set_config_v4(ConfigV4::Dhcp(Default::default()));
    Ok(())
}

#[embassy_executor::task]
pub async fn run(stack: Stack<'static>) -> ! {
    loop {
        while !ENABLED.load(Ordering::Acquire) {
            Timer::after_millis(25).await;
        }
        let mut rx_meta = [PacketMetadata::EMPTY; 1];
        let mut tx_meta = [PacketMetadata::EMPTY; 1];
        let mut rx = [0; 576];
        let mut tx = [0; 576];
        let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx, &mut tx_meta, &mut tx);
        if socket.bind(67).is_err() {
            drop(socket);
            Timer::after_millis(100).await;
            continue;
        }
        let mut leases = DhcpLeasePool::default();
        let peer = critical_section::with(|cs| PEER.borrow(cs).get());
        SERVING.store(true, Ordering::Release);
        while ENABLED.load(Ordering::Acquire) {
            let mut request = [0; 576];
            let mut response = [0; 576];
            if let Ok(Ok((length, _))) =
                with_timeout(Duration::from_millis(100), socket.recv_from(&mut request)).await
            {
                if !ENABLED.load(Ordering::Acquire) {
                    break;
                }
                // Keep the selected MCU at the stable .2 address even if
                // unrelated clients join this public-credential bench network.
                if request.get(28..34) != Some(peer.as_slice()) {
                    continue;
                }
                if let Ok(answer) =
                    answer_appliance_dhcp(&request[..length], &mut leases, &mut response)
                {
                    if let Ok(Ok(())) = with_timeout(
                        Duration::from_secs(1),
                        socket.send_to(
                            &response[..answer.len],
                            IpEndpoint::new(Ipv4Address::BROADCAST.into(), 68),
                        ),
                    )
                    .await
                    {
                        if answer.kind
                            == conduit_rp2040_network_realization::DhcpResponseKind::Acknowledgement
                        {
                            let count = acknowledged();
                            ACKNOWLEDGED.store(count.saturating_add(1), Ordering::Release);
                        }
                    }
                }
            }
        }
        drop(socket);
        SERVING.store(false, Ordering::Release);
    }
}
