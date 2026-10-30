//! Volatile physical carrier only: no Body, Line, admission or execution grant.
use core::{
    cell::{Cell, RefCell},
    fmt::Write,
};
use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use embassy_rp::{peripherals::*, Peri};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration, Instant};
use static_cell::StaticCell;
#[cfg(not(feature = "zenoh-evaluation"))]
#[path = "wifi_mdns.rs"]
mod mdns;
#[path = "wifi_peer.rs"]
mod peer;
#[cfg(not(environment_dht11_tm1637))]
#[path = "../../../firmware/pico-w-signal/src/radio.rs"]
#[allow(unexpected_cfgs, dead_code)]
mod radio;
#[cfg(environment_dht11_tm1637)]
#[path = "radio_environment.rs"]
mod radio;
#[path = "wifi_reconnect.rs"]
mod reconnect;
#[cfg(environment_dht11_tm1637)]
pub(crate) use radio::RadioIrqs as CaptureDmaIrqs;
#[cfg(not(feature = "zenoh-evaluation"))]
#[path = "wifi_transport.rs"]
mod transport;
use peer::Attachment;
pub use radio::UsbIrq as UsbIrqs;
const MAX: usize = conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize;
struct JoinRequest {
    bytes: [u8; MAX],
    length: usize,
    token: u64,
    deadline: Instant,
}
impl Drop for JoinRequest {
    fn drop(&mut self) {
        crate::protocol::wipe(&mut self.bytes)
    }
}
enum Request {
    Join(JoinRequest),
    Peer {
        address: [u8; 4],
        token: u64,
        deadline: Instant,
    },
    #[cfg(not(feature = "zenoh-evaluation"))]
    Http(transport::Transfer),
    #[cfg(not(feature = "zenoh-evaluation"))]
    Mdns(mdns::Observation),
}
static ATTACHMENT: critical_section::Mutex<RefCell<Option<Attachment>>> =
    critical_section::Mutex::new(RefCell::new(None));
fn with_attachment<T>(f: impl FnOnce(&mut Attachment) -> T) -> T {
    critical_section::with(|cs| {
        f(ATTACHMENT
            .borrow(cs)
            .borrow_mut()
            .get_or_insert_with(Attachment::default))
    })
}
fn attachment_snapshot() -> Attachment {
    with_attachment(|a| *a)
}
pub async fn select_peer(bytes: &[u8]) -> Result<(), &'static str> {
    if !critical_section::with(|cs| STATUS.borrow(cs).borrow().ready) {
        return Err("radio-not-ready");
    }

    let address = bytes.try_into().map_err(|_| "peer-address-length")?;
    let token = next_request()?;
    REQUEST.signal(Request::Peer {
        address,
        token,
        deadline: Instant::now() + Duration::from_secs(8),
    });
    with_timeout(Duration::from_secs(8), response(token))
        .await
        .map_err(|_| "peer-selection-timeout")?
}
#[cfg(not(feature = "zenoh-evaluation"))]
pub async fn http(cdc: &mut crate::Cdc, id: u32, bytes: &[u8]) -> Result<(), &'static str> {
    transport::receive(cdc, id, bytes).await
}
#[cfg(not(feature = "zenoh-evaluation"))]
pub async fn mdns(cdc: &mut crate::Cdc, id: u32, bytes: &[u8]) -> Result<(), &'static str> {
    mdns::receive(cdc, id, bytes).await
}
static REQUEST: Signal<CriticalSectionRawMutex, Request> = Signal::new();
static RESULT: Signal<CriticalSectionRawMutex, (u64, Result<(), &'static str>)> = Signal::new();
static NEXT_REQUEST: critical_section::Mutex<Cell<u64>> =
    critical_section::Mutex::new(Cell::new(0));
fn next_request() -> Result<u64, &'static str> {
    critical_section::with(|cs| {
        let next = NEXT_REQUEST
            .borrow(cs)
            .get()
            .checked_add(1)
            .ok_or("radio-request-generation-exhausted")?;
        NEXT_REQUEST.borrow(cs).set(next);
        Ok(next)
    })
}
async fn response(token: u64) -> Result<(), &'static str> {
    loop {
        let (received, result) = RESULT.wait().await;
        if received == token {
            return result;
        }
    }
}
#[derive(Clone, Copy)]
struct Status {
    ready: bool,
    link: bool,
    ip: Option<[u8; 4]>,
    error: Option<&'static str>,
}
static STATUS: critical_section::Mutex<RefCell<Status>> =
    critical_section::Mutex::new(RefCell::new(Status {
        ready: false,
        link: false,
        ip: None,
        error: None,
    }));
fn current(stack: Stack<'static>) -> Option<([u8; 4], u8)> {
    stack
        .is_link_up()
        .then(|| {
            stack
                .config_v4()
                .map(|c| (c.address.address().octets(), c.address.prefix_len()))
        })
        .flatten()
}
fn update(ready: bool, stack: Option<Stack<'static>>, error: Option<&'static str>) {
    critical_section::with(|cs| {
        *STATUS.borrow(cs).borrow_mut() = Status {
            ready,
            link: stack.is_some_and(|s| s.is_link_up()),
            ip: stack
                .filter(|s| s.is_link_up())
                .and_then(|s| s.config_v4().map(|c| c.address.address().octets())),
            error,
        }
    })
}
pub fn status() -> Result<heapless::String<512>, ()> {
    let s = critical_section::with(|cs| *STATUS.borrow(cs).borrow());
    let attachment = attachment_snapshot();
    let mut out = heapless::String::new();
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"body_member\":false,\"radio_ready\":{},\"link_up\":{},\"ip\":",crate::config::ROLE,crate::protocol::BUILD_ID,crate::config::CONFIGURATION_ID,s.ready,s.link).map_err(|_|())?;
    if let Some(ip) = s.ip {
        write!(&mut out, "[{}, {}, {}, {}]", ip[0], ip[1], ip[2], ip[3]).map_err(|_| ())?
    } else {
        out.push_str("null").map_err(|_| ())?
    }
    write!(
        &mut out,
        ",\"attachment_generation\":{},\"peer_ready\":{},\"radio_error\":{}{}{} }}",
        attachment.generation,
        attachment.ready(),
        if s.error.is_some() { "\"" } else { "" },
        s.error.unwrap_or("null"),
        if s.error.is_some() { "\"" } else { "" }
    )
    .map_err(|_| ())?;
    Ok(out)
}
pub async fn configure(encoded: &[u8]) -> Result<(), &'static str> {
    if !critical_section::with(|cs| STATUS.borrow(cs).borrow().ready) {
        return Err("radio-not-ready");
    }
    let selected =
        conduit_net::decode_network_join_request(encoded).map_err(|_| "invalid-network-request")?;
    core::str::from_utf8(selected.credential).map_err(|_| "unsupported-network-credential")?;
    let mut request = JoinRequest {
        bytes: [0; MAX],
        length: encoded.len(),
        token: next_request()?,
        deadline: Instant::now() + Duration::from_secs(50),
    };
    request.bytes[..encoded.len()].copy_from_slice(encoded);
    let token = request.token;
    REQUEST.signal(Request::Join(request));
    with_timeout(Duration::from_secs(50), response(token))
        .await
        .map_err(|_| "radio-request-timeout")?
}
static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
static FW: aligned::Aligned<aligned::A4, [u8; 231077]> = aligned::Aligned(*include_bytes!(
    "../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static NVRAM:aligned::Aligned<aligned::A4,[u8;742]>=aligned::Aligned(*include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"));
static CLM:&[u8]=include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin");
#[embassy_executor::task]
async fn network(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}
#[embassy_executor::task]
pub async fn run(
    spawner: Spawner,
    pio: Peri<'static, PIO0>,
    tx: Peri<'static, DMA_CH0>,
    rx: Peri<'static, DMA_CH1>,
    power: Peri<'static, PIN_23>,
    dio: Peri<'static, PIN_24>,
    cs: Peri<'static, PIN_25>,
    clock: Peri<'static, PIN_29>,
) {
    let initialized = radio::init_cyw43_network(
        &spawner, pio, tx, rx, power, dio, cs, clock, &FW, &NVRAM, CLM,
    )
    .await;
    let Ok((device, mut control)) = initialized else {
        update(false, None, Some("radio-initialization-failed"));
        loop {
            let request = REQUEST.wait().await;
            match request {
                #[cfg(not(feature = "zenoh-evaluation"))]
                Request::Http(transfer) => transport::unavailable(transfer).await,
                #[cfg(not(feature = "zenoh-evaluation"))]
                Request::Mdns(request) => mdns::unavailable(request),
                Request::Join(request) => {
                    RESULT.signal((request.token, Err("radio-initialization-failed")))
                }
                Request::Peer { token, .. } => {
                    RESULT.signal((token, Err("radio-initialization-failed")))
                }
            }
        }
    };
    // Nonsecret network transaction entropy; this is not a membership seed.
    let mut rng = embassy_rp::clocks::RoscRng;
    let (stack, runner) = embassy_net::new(
        device,
        embassy_net::Config::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        rng.next_u64(),
    );
    spawner.spawn(network(runner).unwrap());
    #[cfg(feature = "zenoh-evaluation")]
    spawner.spawn(crate::zenoh_evaluation::network(stack).unwrap());
    update(true, Some(stack), None);
    spawner.spawn(monitor(stack).unwrap());
    #[cfg(not(feature = "zenoh-evaluation"))]
    for _ in 0..crate::environment_http::SESSIONS {
        spawner.spawn(crate::environment_http::run(stack).unwrap());
    }
    let mut retained: Option<JoinRequest> = None;
    let mut reconnect = reconnect::Reconnect::default();
    loop {
        if let Some(request) = retained.as_ref() {
            let usable = current(stack).is_some();
            if reconnect.poll(
                embassy_time::Instant::now().as_millis(),
                usable,
                stack.is_link_up(),
            ) {
                let result = match with_attachment(|a| a.invalidate()) {
                    Ok(()) => join(&mut control, stack, &request.bytes[..request.length]).await,
                    Err(e) => Err(e),
                };
                reconnect.completed(embassy_time::Instant::now().as_millis(), result.is_ok());
                update(true, Some(stack), result.err());
            }
        }
        let request = match with_timeout(Duration::from_millis(250), REQUEST.wait()).await {
            Ok(r) => r,
            Err(_) => {
                continue;
            }
        };
        match request {
            Request::Join(request) => {
                // Never begin a credential change that cannot finish within its caller's budget.
                if !reconnect::can_start(Instant::now().as_millis(), request.deadline.as_millis()) {
                    RESULT.signal((request.token, Err("radio-request-expired")));
                    continue;
                }
                let token = request.token;
                // Stop using and wipe superseded credentials before another join.
                drop(retained.take());
                let retired = with_attachment(|a| a.invalidate());
                let result = match retired {
                    Ok(()) => join(&mut control, stack, &request.bytes[..request.length]).await,
                    Err(e) => Err(e),
                };
                reconnect = reconnect::Reconnect::default();
                reconnect.completed(embassy_time::Instant::now().as_millis(), result.is_ok());
                // Replacement drops and wipes the previous fixed-capacity request.
                retained = Some(request);
                let _ = with_attachment(|a| a.observe(current(stack)));
                update(true, Some(stack), result.as_ref().err().copied());
                RESULT.signal((token, result));
            }
            Request::Peer {
                address,
                token,
                deadline,
            } => {
                if Instant::now() >= deadline {
                    RESULT.signal((token, Err("peer-selection-timeout")));
                    continue;
                }
                let result = with_attachment(|a| {
                    a.observe(current(stack))?;
                    a.select(address)
                });
                RESULT.signal((token, result));
            }
            #[cfg(not(feature = "zenoh-evaluation"))]
            Request::Mdns(request) => mdns::serve(stack, request).await,
            #[cfg(not(feature = "zenoh-evaluation"))]
            Request::Http(transfer) => {
                let _ = with_attachment(|a| a.observe(current(stack)));
                transport::serve(stack, &attachment_snapshot(), transfer).await;
            }
        }
    }
}
async fn join(
    control: &mut cyw43::Control<'static>,
    stack: Stack<'static>,
    encoded: &[u8],
) -> Result<(), &'static str> {
    let r =
        conduit_net::decode_network_join_request(encoded).map_err(|_| "invalid-network-request")?;
    let ssid = core::str::from_utf8(r.ssid).map_err(|_| "invalid-network-request")?;
    let credential =
        core::str::from_utf8(r.credential).map_err(|_| "unsupported-network-credential")?;
    with_timeout(Duration::from_secs(5), control.leave())
        .await
        .map_err(|_| "radio-leave-timeout")?;
    stack.set_config_v4(embassy_net::ConfigV4::Dhcp(Default::default()));
    // The private carrier profile admits WPA2 Personal only. The pinned CYW43
    // default selects WPA3, so it must not override that explicit policy.
    let mut options = cyw43::JoinOptions::new(credential.as_bytes());
    options.auth = cyw43::JoinAuth::Wpa2;
    with_timeout(Duration::from_secs(25), control.join(ssid, options))
        .await
        .map_err(|_| "radio-join-timeout")?
        .map_err(|_| "radio-join-refused")?;
    with_timeout(Duration::from_secs(15), stack.wait_config_up())
        .await
        .map_err(|_| "radio-dhcp-timeout")?;
    Ok(())
}

#[embassy_executor::task]
async fn monitor(stack: Stack<'static>) -> ! {
    loop {
        let observed = with_attachment(|a| a.observe(current(stack)));
        let old = critical_section::with(|cs| STATUS.borrow(cs).borrow().error);
        let error = if current(stack).is_some() && reconnect::network_error(old) {
            observed.err()
        } else {
            observed.err().or(old)
        };
        update(observed.is_ok(), Some(stack), error);
        embassy_time::Timer::after_millis(50).await;
    }
}
