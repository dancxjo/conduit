//! Same pinned CYW43 initializer with one combined CH0/1/2 DMA vector.
//! DHT and radio own distinct channels; this creates no Body/Plot authority.
use cyw43::Control;
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, dma,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, DMA_CH1, DMA_CH2, PIN_23, PIN_24, PIN_25, PIN_29, PIO0, USB},
    pio::Pio,
    usb, Peri,
};
use embassy_time::{with_timeout, Duration};
use static_cell::StaticCell;
bind_interrupts!(pub struct UsbIrq{USBCTRL_IRQ=>usb::InterruptHandler<USB>;});
bind_interrupts!(pub struct RadioIrqs{PIO0_IRQ_0=>embassy_rp::pio::InterruptHandler<PIO0>;DMA_IRQ_0=>dma::InterruptHandler<DMA_CH0>,dma::InterruptHandler<DMA_CH1>,dma::InterruptHandler<DMA_CH2>;});
static STATE: StaticCell<cyw43::State> = StaticCell::new();
const RADIO_PHASE_TIMEOUT: Duration = Duration::from_secs(20);
pub enum NetworkRadioInitError {
    DriverStartupTimeout,
    InitializationTimeout,
}
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
        cyw43::Cyw43439,
    >,
) -> ! {
    runner.run().await
}
#[allow(clippy::too_many_arguments)]
pub async fn init_cyw43_network(
    spawner: &Spawner,
    pio0: Peri<'static, PIO0>,
    dma_ch0: Peri<'static, DMA_CH0>,
    dma_ch1: Peri<'static, DMA_CH1>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
    clm: &'static [u8],
) -> Result<(cyw43::NetDriver<'static>, Control<'static>), NetworkRadioInitError> {
    let pwr = Output::new(pin23, Level::Low);
    let cs = Output::new(pin25, Level::High);
    let mut pio = Pio::new(pio0, RadioIrqs);
    let dma_tx = dma::Channel::new(dma_ch0, RadioIrqs);
    let dma_rx = dma::Channel::new(dma_ch1, RadioIrqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        cyw43_pio::DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        pin24,
        pin29,
        dma_tx,
        dma_rx,
    );
    let state = STATE.init(cyw43::State::new());
    let driver_startup =
        with_timeout(RADIO_PHASE_TIMEOUT, cyw43::new(state, pwr, spi, fw, nvram)).await;
    let (net_device, mut control, runner) = match driver_startup {
        Ok(driver) => driver,
        Err(_) => {
            return Err(NetworkRadioInitError::DriverStartupTimeout);
        }
    };
    spawner.spawn(cyw43_task(runner).unwrap());

    let initialization = with_timeout(RADIO_PHASE_TIMEOUT, async {
        control.init(clm).await;
        control
            .set_power_management(cyw43::PowerManagementMode::None)
            .await;
    })
    .await;
    if initialization.is_err() {
        return Err(NetworkRadioInitError::InitializationTimeout);
    }
    Ok((net_device, control))
}
