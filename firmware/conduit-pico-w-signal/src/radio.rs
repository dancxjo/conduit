//! CYW43 radio initialisation for the Pico W onboard LED.
//!
//! Wiring follows the proven Pico W radio pinout:
//!   PIO0 / SM0 / DMA_CH0
//!   PIN_23 (power), PIN_24 (DIO), PIN_25 (CS), PIN_29 (clock)

use cyw43::Control;
use cyw43_pio::{PioSpi, DEFAULT_CLOCK_DIVIDER};
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    dma,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0, USB},
    pio::Pio,
    usb,
    Peri,
};
#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
))]
use embassy_time::{with_timeout, Duration};
use static_cell::StaticCell;

bind_interrupts!(pub struct UsbIrq {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

bind_interrupts!(struct RadioIrqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

static STATE: StaticCell<cyw43::State> = StaticCell::new();

#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
))]
const RADIO_PHASE_TIMEOUT: Duration = Duration::from_secs(20);

#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
))]
pub enum NetworkRadioInitError {
    DriverStartupTimeout,
    InitializationTimeout,
}

#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
))]
impl NetworkRadioInitError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DriverStartupTimeout => "radio-driver-startup-timeout",
            Self::InitializationTimeout => "radio-initialization-timeout",
        }
    }
}

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

#[allow(
    clippy::too_many_arguments,
    reason = "the Pico W radio boundary names each fixed peripheral and asset explicitly"
)]
#[cfg(not(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
)))]
pub async fn init_cyw43(
    spawner: &Spawner,
    pio0: Peri<'static, PIO0>,
    dma_ch0: Peri<'static, DMA_CH0>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
) -> (Control<'static>, ()) {
    let pwr = Output::new(pin23, Level::Low);
    let cs = Output::new(pin25, Level::High);
    let mut pio = Pio::new(pio0, RadioIrqs);
    let dma = dma::Channel::new(dma_ch0, RadioIrqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        pin24,
        pin29,
        dma,
    );
    let state = STATE.init(cyw43::State::new());
    let (_net_device, control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(cyw43_task(runner).unwrap());
    (control, ())
}

#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client"
))]
#[allow(
    clippy::too_many_arguments,
    reason = "the Pico W network boundary names each fixed peripheral and asset explicitly"
)]
pub async fn init_cyw43_network(
    spawner: &Spawner,
    pio0: Peri<'static, PIO0>,
    dma_ch0: Peri<'static, DMA_CH0>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
    clm: &'static [u8],
) -> Result<(cyw43::NetDriver<'static>, Control<'static>), NetworkRadioInitError> {
    #[cfg(feature = "wifi-bootstrap")]
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::RadioDriverStartup);
    let pwr = Output::new(pin23, Level::Low);
    let cs = Output::new(pin25, Level::High);
    let mut pio = Pio::new(pio0, RadioIrqs);
    let dma = dma::Channel::new(dma_ch0, RadioIrqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        pin24,
        pin29,
        dma,
    );
    let state = STATE.init(cyw43::State::new());
    let driver_startup = with_timeout(
        RADIO_PHASE_TIMEOUT,
        cyw43::new(state, pwr, spi, fw, nvram),
    )
    .await;
    let (net_device, mut control, runner) = match driver_startup {
        Ok(driver) => driver,
        Err(_) => {
            #[cfg(feature = "wifi-bootstrap")]
            crate::panic_recovery::clear();
            return Err(NetworkRadioInitError::DriverStartupTimeout);
        }
    };
    spawner.spawn(cyw43_task(runner).unwrap());
    #[cfg(feature = "wifi-bootstrap")]
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::RadioInitialization);
    let initialization = with_timeout(RADIO_PHASE_TIMEOUT, async {
        control.init(clm).await;
        control
            .set_power_management(cyw43::PowerManagementMode::None)
            .await;
    })
    .await;
    if initialization.is_err() {
        #[cfg(feature = "wifi-bootstrap")]
        crate::panic_recovery::clear();
        return Err(NetworkRadioInitError::InitializationTimeout);
    }
    Ok((net_device, control))
}
