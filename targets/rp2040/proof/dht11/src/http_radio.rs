//! Physical AP composition reusing the existing Pico appliance radio sequence.
use aligned::{Aligned, A4};
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, dma,
    gpio::{Level, Output},
    peripherals::*,
    pio::Pio,
    Peri,
};
use embassy_time::{with_timeout, Duration};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});
extern crate alloc;
use alloc::boxed::Box;
use core::{future::Future, pin::Pin};
pub struct Resources {
    pub spawner: Spawner,
    pub pio: Peri<'static, PIO0>,
    pub tx: Peri<'static, DMA_CH0>,
    pub rx: Peri<'static, DMA_CH1>,
    pub power: Peri<'static, PIN_23>,
    pub data: Peri<'static, PIN_24>,
    pub cs: Peri<'static, PIN_25>,
    pub clock: Peri<'static, PIN_29>,
}
#[embassy_executor::task]
async fn service(future: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    future.await;
}
/// Prepare network storage only after the Plan/Back preparation peak. It remains
/// retained for the appliance lifetime; no allocation is permitted during Play.
#[inline(never)]
pub fn launch(resources: Resources) {
    let spawner = resources.spawner;
    let state = Box::leak(Box::new(cyw43::State::new()));
    let buffers = Box::leak(Box::new(embassy_net::StackResources::<3>::new()));
    crate::http_view::initialize();
    let future = Box::pin(start(resources, state, buffers));
    spawner.spawn(service(future).unwrap());
}
static FW: Aligned<A4, [u8; 231077]> = Aligned(*include_bytes!(
    "../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static NVRAM: Aligned<A4, [u8; 742]> = Aligned(*include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"));
static CLM: &[u8] = include_bytes!("../../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin");

#[embassy_executor::task]
async fn network_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

async fn start(
    resources: Resources,
    state: &'static mut cyw43::State,
    buffers: &'static mut embassy_net::StackResources<3>,
) {
    let Resources {
        spawner,
        pio,
        tx,
        rx,
        power,
        data,
        cs,
        clock,
    } = resources;
    let mut pio = Pio::new(pio, Irqs);
    let spi = cyw43_pio::PioSpi::new(
        &mut pio.common,
        pio.sm0,
        cyw43_pio::DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        Output::new(cs, Level::High),
        data,
        clock,
        dma::Channel::new(tx, crate::CaptureIrqs),
        dma::Channel::new(rx, crate::CaptureIrqs),
    );
    let (device, mut control, runner) = with_timeout(
        Duration::from_secs(20),
        cyw43::new(state, Output::new(power, Level::Low), spi, &FW, &NVRAM),
    )
    .await
    .expect("radio startup timeout");
    embassy_futures::join::join(runner.run(), async {
        with_timeout(Duration::from_secs(20), control.init(CLM))
            .await
            .expect("radio init timeout");
        control
            .set_power_management(cyw43::PowerManagementMode::None)
            .await;
        let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
            address: embassy_net::Ipv4Cidr::new(embassy_net::Ipv4Address::new(192, 168, 4, 1), 24),
            gateway: None,
            dns_servers: Default::default(),
        });
        let seed = embassy_rp::clocks::RoscRng.next_u64();
        let (stack, runner) = embassy_net::new(device, config, buffers, seed);
        spawner.spawn(network_task(runner).unwrap());
        with_timeout(
            Duration::from_secs(10),
            control.start_ap_open("Conduit-Climate", 6),
        )
        .await
        .expect("AP startup timeout");
        crate::http_view::set_ap_ready();
        crate::http_ap::serve(stack).await;
    })
    .await;
}
