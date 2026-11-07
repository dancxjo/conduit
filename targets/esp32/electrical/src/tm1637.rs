use conduit_tm1637_wire::OpenDrain;
use core::convert::Infallible;
use esp_hal::{
    delay::Delay,
    gpio::{Flex, InputConfig, Pin, Pull},
};

/// Owned pin pair for the ordinary bounded TM1637 transaction mechanism.
/// The Mask supplies registers; this provider does not invent display content.
pub struct Esp32Tm1637<'d> {
    clock: Flex<'d>,
    data: Flex<'d>,
    delay: Delay,
    half_period_us: u16,
}
impl<'d> Esp32Tm1637<'d> {
    /// Reject timing before constructing/changing either pin. The composition
    /// root supplies distinct owned pins and its checked physical attachment.
    pub fn new(
        clock: impl Pin + 'd,
        data: impl Pin + 'd,
        half_period_us: u16,
        weak_pullups: bool,
    ) -> Option<Self> {
        if !(5..=1000).contains(&half_period_us) {
            return None;
        }
        let mut clock = Flex::new(clock);
        let mut data = Flex::new(data);
        for pin in [&mut clock, &mut data] {
            pin.set_output_enable(false);
            pin.set_low();
            pin.set_input_enable(true);
            pin.apply_input_config(&InputConfig::default().with_pull(if weak_pullups {
                Pull::Up
            } else {
                Pull::None
            }));
        }
        Some(Self {
            clock,
            data,
            delay: Delay::new(),
            half_period_us,
        })
    }
}
impl OpenDrain for Esp32Tm1637<'_> {
    type Error = Infallible;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        self.clock.set_output_enable(!released);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), Self::Error> {
        self.data.set_output_enable(!released);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(self.data.is_high())
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        self.delay.delay_micros(u32::from(self.half_period_us));
        Ok(())
    }
}
impl Drop for Esp32Tm1637<'_> {
    fn drop(&mut self) {
        self.clock.set_output_enable(false);
        self.data.set_output_enable(false);
    }
}
