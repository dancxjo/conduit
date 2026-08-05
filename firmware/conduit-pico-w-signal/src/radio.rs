//! CYW43 radio initialisation for the Pico W onboard LED.
//!
//! Wiring follows the proven Pico W radio pinout:
//!   PIO0 / SM0 / DMA_CH0
//!   PIN_23 (power), PIN_24 (DIO), PIN_25 (CS), PIN_29 (clock)

use cyw43::Control;
use cyw43_pio::PioSpi;
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
use fixed::{types::extra::U8, FixedU32};
use static_cell::StaticCell;

bind_interrupts!(pub struct UsbIrq {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

bind_interrupts!(struct PioIrq {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});

bind_interrupts!(struct DmaIrq {
    DMA_IRQ_0 => dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

static STATE: StaticCell<cyw43::State> = StaticCell::new();

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

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
    let mut pio = Pio::new(pio0, PioIrq);
    let dma = dma::Channel::new(dma_ch0, DmaIrq);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        FixedU32::<U8>::from_num(2u32),
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
