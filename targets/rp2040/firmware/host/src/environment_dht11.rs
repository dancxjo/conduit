//! GPIO equipment initialization only. Measurement requires an admitted Play.
use crate::config;
use core::fmt::Write;
use embassy_rp::{
    peripherals::{DMA_CH2, PIO1},
    Peri,
};
embassy_rp::bind_interrupts!(struct CaptureIrqs {
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO1>;
});
pub fn initialize(
    pio: Peri<'static, PIO1>,
    dma: Peri<'static, DMA_CH2>,
    data: Peri<'static, config::SensorData>,
    clock: Peri<'static, config::DisplayClock>,
    display_data: Peri<'static, config::DisplayData>,
) {
    let mut pio = embassy_rp::pio::Pio::new(pio, CaptureIrqs);
    let program = conduit_rp2040_pulse_capture::PulseProgram::load(&mut pio.common);
    crate::pulse_owner::initialize(conduit_rp2040_pulse_capture::PicoPulseCapture::new(
        &mut pio.common,
        pio.sm0,
        embassy_rp::dma::Channel::new(dma, crate::radio_irqs::RadioIrqs),
        data,
        program,
    ));
    crate::tm1637_owner::initialize(crate::tm1637_pins::Pins::new(clock, display_data));
}
pub fn status() -> Result<heapless::String<1024>, ()> {
    let mut out = heapless::String::new();
    let (_, peak) = crate::heap::usage();
    let (count, bytes, _) = crate::resident::summary();
    write!(&mut out, "{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"sensor\":\"dht11\",\"display\":\"tm1637\",\"heap_peak\":{},\"initialized_resident_backs\":{},\"resident_back_bytes\":{},\"body_member\":{}", config::ROLE, crate::BUILD_ID, config::CONFIGURATION_ID, peak, count, bytes, crate::membership::is_member()).map_err(|_| ())?;
    crate::membership::append_status(&mut out)?;
    out.push('}').map_err(|_| ())?;
    Ok(out)
}
