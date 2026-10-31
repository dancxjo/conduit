//! Station-scoped IPv4 hostname mDNS. Backing is admitted before serving AP.
#[path = "native_mdns_wire.rs"]
mod wire;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_futures::select::{select, Either};
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    IpAddress, IpEndpoint, Ipv4Address, Stack,
};
use embassy_time::{Duration, Instant, Timer};
static AVAILABLE: AtomicBool = AtomicBool::new(false);
static SUFFIX: AtomicU8 = AtomicU8::new(0);
pub struct Buffers {
    rx_meta: [PacketMetadata; 1],
    rx: [u8; 512],
    tx_meta: [PacketMetadata; 1],
    tx: [u8; 512],
    reply: [u8; 512],
}
impl Buffers {
    pub fn new() -> Self {
        Self {
            rx_meta: [PacketMetadata::EMPTY; 1],
            rx: [0; 512],
            tx_meta: [PacketMetadata::EMPTY; 1],
            tx: [0; 512],
            reply: [0; 512],
        }
    }
}
pub const BACKING_BYTES: usize = core::mem::size_of::<Buffers>();
pub fn available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}
pub fn hostname() -> heapless::String<48> {
    label(SUFFIX.load(Ordering::Acquire))
}
fn label(suffix: u8) -> heapless::String<48> {
    use core::fmt::Write;
    let mac = esp_hal::efuse::interface_mac_address(esp_hal::efuse::InterfaceMacAddress::Station);
    let mut name = heapless::String::new();
    name.push_str("conduit-climate-").unwrap();
    for octet in mac.as_bytes() {
        write!(name, "{octet:02x}").unwrap();
    }
    if suffix != 0 {
        write!(name, "-{}", suffix + 1).unwrap();
    }
    name
}
struct Membership(Stack<'static>);
impl Drop for Membership {
    fn drop(&mut self) {
        AVAILABLE.store(false, Ordering::Release);
        let _ = self
            .0
            .leave_multicast_group(Ipv4Address::new(224, 0, 0, 251));
    }
}
#[derive(Clone, Copy)]
struct Pending {
    length: usize,
    destination: IpEndpoint,
    source: IpEndpoint,
    deadline: Instant,
    multicast: bool,
}
#[derive(Clone, Copy)]
enum Phase {
    Probe(u8),
    Announce,
    Ready,
}
fn jitter(maximum: u32) -> Duration {
    Duration::from_millis(u64::from(esp_hal::rng::Rng::new().random() % maximum))
}
pub async fn run(stack: Stack<'static>, buffers: &mut Buffers) -> Result<(), ()> {
    let initial = stack.config_v4().ok_or(())?;
    if !stack.is_link_up() {
        return Err(());
    }
    let address = initial.address.address().octets();
    let multicast = IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::new(224, 0, 0, 251)), 5353);
    stack
        .join_multicast_group(Ipv4Address::new(224, 0, 0, 251))
        .map_err(|_| ())?;
    let _membership = Membership(stack);
    let Buffers {
        rx_meta,
        rx,
        tx_meta,
        tx,
        reply,
    } = buffers;
    let mut socket = UdpSocket::new(stack, rx_meta, rx, tx_meta, tx);
    socket.bind(5353).map_err(|_| ())?;
    socket.set_hop_limit(Some(255));
    let mut suffix = 0;
    let mut name = wire::Name::new(&label(suffix)).map_err(|_| ())?;
    let mut phase = Phase::Probe(0);
    let mut due = Instant::now() + jitter(251);
    let mut pending: Option<Pending> = None;
    let mut last_multicast: Option<Instant> = None;
    loop {
        if !stack.is_link_up()
            || stack
                .config_v4()
                .is_none_or(|config| config.address != initial.address)
        {
            return Err(());
        }
        let now = Instant::now();
        let deadline = if let Some(pending) = pending {
            pending.deadline.min(now + Duration::from_secs(1))
        } else if matches!(phase, Phase::Ready) {
            now + Duration::from_secs(1)
        } else {
            due
        };
        let received = socket.recv_from_with(|packet, metadata| {
            let IpAddress::Ipv4(source) = metadata.endpoint.addr;
            if !initial.address.contains_addr(&source) {
                return (false, None, false);
            }
            let Ok(interest) = wire::inspect(packet, &name, address) else {
                return (false, None, false);
            };
            if interest.conflict {
                return (true, None, false);
            }
            let suppress = interest.suppress
                && pending.is_some_and(|old| old.source == metadata.endpoint && old.multicast);
            if interest.question
                && !interest.suppress
                && matches!(phase, Phase::Ready)
                && pending.is_none()
            {
                let legacy = metadata.endpoint.port != 5353;
                if let Ok(length) =
                    wire::answer(&name, address, legacy.then_some((packet, interest)), reply)
                {
                    let unicast = legacy || interest.unicast;
                    let now = Instant::now();
                    let mut deadline = now
                        + if interest.truncated {
                            Duration::from_millis(500)
                        } else {
                            Duration::from_millis(20) + jitter(101)
                        };
                    if !unicast {
                        if let Some(last) = last_multicast {
                            deadline = deadline.max(last + Duration::from_secs(1));
                        }
                    }
                    return (
                        false,
                        Some(Pending {
                            length,
                            destination: if unicast {
                                metadata.endpoint
                            } else {
                                multicast
                            },
                            source: metadata.endpoint,
                            deadline,
                            multicast: !unicast,
                        }),
                        suppress,
                    );
                }
            }
            (false, None, suppress)
        });
        // Give due timers priority even on a busy multicast link.
        let event = match select(Timer::at(deadline), received).await {
            Either::First(()) => Either::Second(()),
            Either::Second(packet) => Either::First(packet),
        };
        match event {
            Either::First((conflict, next, suppress)) => {
                if conflict {
                    AVAILABLE.store(false, Ordering::Release);
                    suffix += 1;
                    if suffix > 2 {
                        return Err(());
                    }
                    name = wire::Name::new(&label(suffix)).map_err(|_| ())?;
                    phase = Phase::Probe(0);
                    due = Instant::now() + Duration::from_secs(1) + jitter(251);
                    pending = None;
                } else if suppress {
                    pending = None;
                } else if next.is_some() {
                    pending = next;
                }
            }
            Either::Second(()) => {
                let now = Instant::now();
                if let Some(ready) = pending.filter(|next| now >= next.deadline) {
                    socket
                        .send_to(&reply[..ready.length], ready.destination)
                        .await
                        .map_err(|_| ())?;
                    if ready.multicast {
                        last_multicast = Some(Instant::now());
                    }
                    pending = None;
                }
                if now >= due {
                    match phase {
                        Phase::Probe(sent) if sent < 3 => {
                            let length = wire::probe(&name, address, reply).map_err(|_| ())?;
                            socket
                                .send_to(&reply[..length], multicast)
                                .await
                                .map_err(|_| ())?;
                            phase = Phase::Probe(sent + 1);
                            due = Instant::now() + Duration::from_millis(250);
                        }
                        Phase::Probe(_) => {
                            let length =
                                wire::answer(&name, address, None, reply).map_err(|_| ())?;
                            socket
                                .send_to(&reply[..length], multicast)
                                .await
                                .map_err(|_| ())?;
                            last_multicast = Some(Instant::now());
                            SUFFIX.store(suffix, Ordering::Release);
                            AVAILABLE.store(true, Ordering::Release);
                            phase = Phase::Announce;
                            due = Instant::now() + Duration::from_secs(1);
                        }
                        Phase::Announce => {
                            let length =
                                wire::answer(&name, address, None, reply).map_err(|_| ())?;
                            socket
                                .send_to(&reply[..length], multicast)
                                .await
                                .map_err(|_| ())?;
                            last_multicast = Some(Instant::now());
                            phase = Phase::Ready;
                        }
                        Phase::Ready => (),
                    }
                }
            }
        }
        // One bounded packet/timer action per cooperative poll prevents a busy
        // link from monopolizing the sampler's executor.
        embassy_futures::yield_now().await;
    }
}
