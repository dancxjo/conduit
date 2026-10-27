//! Volatile physical carrier only: no Body, Line, admission or execution grant.
use core::{cell::RefCell, fmt::Write};
use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use embassy_rp::{peripherals::*, Peri};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration};
use static_cell::StaticCell;
#[path = "../../../firmware/pico-w-signal/src/radio.rs"]
#[allow(unexpected_cfgs, dead_code)]
mod radio;
pub use radio::UsbIrq as UsbIrqs;
const MAX: usize = conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize;
struct Request {
    bytes: [u8; MAX],
    length: usize,
}
impl Drop for Request {
    fn drop(&mut self) {
        crate::protocol::wipe(&mut self.bytes)
    }
}
static REQUEST: Signal<CriticalSectionRawMutex, Request> = Signal::new();
static RESULT: Signal<CriticalSectionRawMutex, Result<(), &'static str>> = Signal::new();
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
    let mut out = heapless::String::new();
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"body_member\":false,\"radio_ready\":{},\"link_up\":{},\"ip\":",crate::config::ROLE,crate::protocol::BUILD_ID,crate::config::CONFIGURATION_ID,s.ready,s.link).map_err(|_|())?;
    if let Some(ip) = s.ip {
        write!(&mut out, "[{}, {}, {}, {}]", ip[0], ip[1], ip[2], ip[3]).map_err(|_| ())?
    } else {
        out.push_str("null").map_err(|_| ())?
    }
    write!(
        &mut out,
        ",\"radio_error\":{}{}{} }}",
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
    let mut request = Request {
        bytes: [0; MAX],
        length: encoded.len(),
    };
    request.bytes[..encoded.len()].copy_from_slice(encoded);
    RESULT.reset();
    REQUEST.signal(request);
    with_timeout(Duration::from_secs(50), RESULT.wait())
        .await
        .map_err(|_| "radio-request-timeout")?
}
static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
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
            drop(request);
            RESULT.signal(Err("radio-initialization-failed"));
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
    update(true, Some(stack), None);
    loop {
        let request = match with_timeout(Duration::from_millis(250), REQUEST.wait()).await {
            Ok(r) => r,
            Err(_) => {
                let old = critical_section::with(|cs| STATUS.borrow(cs).borrow().error);
                update(true, Some(stack), old);
                continue;
            }
        };
        let result = join(&mut control, stack, &request.bytes[..request.length]).await;
        drop(request);
        update(true, Some(stack), result.as_ref().err().copied());
        RESULT.signal(result);
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
    with_timeout(
        Duration::from_secs(25),
        control.join(ssid, cyw43::JoinOptions::new(credential.as_bytes())),
    )
    .await
    .map_err(|_| "radio-join-timeout")?
    .map_err(|_| "radio-join-refused")?;
    with_timeout(Duration::from_secs(15), stack.wait_config_up())
        .await
        .map_err(|_| "radio-dhcp-timeout")?;
    Ok(())
}
