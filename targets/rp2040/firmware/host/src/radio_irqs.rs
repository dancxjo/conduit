//! One physical DMA vector for precisely the selected cold equipment owners.
//! Capture keeps DMA2 service even when no retained radio is constructed.
#[cfg(all(retained_wifi, not(sensor_dht11)))]
embassy_rp::bind_interrupts!(pub(crate) struct RadioIrqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>, embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH1>;
});
#[cfg(all(retained_wifi, sensor_dht11))]
embassy_rp::bind_interrupts!(pub(crate) struct RadioIrqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>, embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH1>, embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});
#[cfg(all(not(retained_wifi), sensor_dht11))]
embassy_rp::bind_interrupts!(pub(crate) struct RadioIrqs {
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});
