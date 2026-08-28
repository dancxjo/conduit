//! Independent first-stage Pico W onboard LED heartbeat.

use aligned::{Aligned, A4};
use embassy_executor::Spawner;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::Peri;
use embassy_time::{Duration, Timer};
use portable_atomic::{AtomicBool, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

#[embassy_executor::task]
#[allow(
    clippy::too_many_arguments,
    reason = "the Pico W heartbeat owns each fixed CYW43 peripheral explicitly"
)]
pub async fn task(
    spawner: Spawner,
    pio0: Peri<'static, PIO0>,
    dma_ch0: Peri<'static, DMA_CH0>,
    dma_ch1: Peri<'static, DMA_CH1>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static Aligned<A4, [u8]>,
    nvram: &'static Aligned<A4, [u8]>,
) {
    let (mut control, _) = crate::radio::init_cyw43(
        &spawner, pio0, dma_ch0, dma_ch1, pin23, pin24, pin25, pin29, fw, nvram,
    )
    .await;
    INITIALIZED.store(true, Ordering::Release);
    loop {
        control.gpio_set(0, true).await;
        Timer::after(Duration::from_millis(200)).await;
        control.gpio_set(0, false).await;
        Timer::after(Duration::from_millis(800)).await;
    }
}
