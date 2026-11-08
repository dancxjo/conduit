//! Existing retained network attachment and read-only diagnostics, no Body Line.
use crate::credential_source::Credentials;
use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use embassy_rp::{peripherals::*, Peri};
use embassy_time::{with_timeout, Duration, Timer};
use static_cell::StaticCell;
static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
static FW: aligned::Aligned<aligned::A4, [u8; 231077]> = aligned::Aligned(*include_bytes!(
    "../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static NVRAM: aligned::Aligned<aligned::A4, [u8; 742]> = aligned::Aligned(*include_bytes!(
    "../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"
));
static CLM: &[u8] = include_bytes!(
    "../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin"
);
#[embassy_executor::task]
async fn network(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}
#[embassy_executor::task]
#[allow(
    clippy::too_many_arguments,
    reason = "fixed onboard peripheral ownership is explicit"
)]
pub async fn run(
    spawner: Spawner,
    pio: Peri<'static, PIO0>,
    tx: Peri<'static, DMA_CH0>,
    rx: Peri<'static, DMA_CH1>,
    power: Peri<'static, PIN_23>,
    dio: Peri<'static, PIN_24>,
    cs: Peri<'static, PIN_25>,
    clock: Peri<'static, PIN_29>,
    credentials: Option<Credentials>,
) {
    let Ok((device, mut control)) = crate::radio::init_cyw43_network(
        &spawner, pio, tx, rx, power, dio, cs, clock, &FW, &NVRAM, CLM,
    )
    .await
    else {
        return;
    };
    let mut rng = embassy_rp::clocks::RoscRng;
    let (stack, runner) = embassy_net::new(
        device,
        embassy_net::Config::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        rng.next_u64(),
    );
    spawner.spawn(network(runner).unwrap());
    // Shared Source execution exposes its real Body Face through admitted
    // carriers; do not retain the separate diagnostic environmental server.
    #[cfg(not(shared_source_execution))]
    for _ in 0..crate::environment_http::SESSIONS {
        spawner.spawn(crate::environment_http::run(stack).unwrap());
    }
    let Some(credentials) = credentials else {
        return;
    };
    let mut failures = 0u8;
    loop {
        if stack.is_link_up() && stack.config_v4().is_some() {
            Timer::after_secs(1).await;
            continue;
        }
        let usable = join(
            &mut control,
            stack,
            &credentials.bytes[..credentials.length],
        )
        .await
        .is_ok();
        failures = if usable {
            0
        } else {
            failures.saturating_add(1)
        };
        let delay = if usable {
            1
        } else {
            (5u64 << failures.saturating_sub(1).min(4)).min(60)
        };
        Timer::after_secs(delay).await;
    }
}
async fn join(
    control: &mut cyw43::Control<'static>,
    stack: Stack<'static>,
    bytes: &[u8],
) -> Result<(), ()> {
    let r = conduit_net::decode_network_join_request(bytes).map_err(|_| ())?;
    let ssid = core::str::from_utf8(r.ssid).map_err(|_| ())?;
    with_timeout(Duration::from_secs(5), control.leave())
        .await
        .map_err(|_| ())?;
    stack.set_config_v4(embassy_net::ConfigV4::Dhcp(Default::default()));
    let mut options = cyw43::JoinOptions::new(r.credential);
    options.auth = cyw43::JoinAuth::Wpa2;
    with_timeout(Duration::from_secs(25), control.join(ssid, options))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    with_timeout(Duration::from_secs(15), stack.wait_config_up())
        .await
        .map_err(|_| ())?;
    Ok(())
}
