//! Owned open-drain diagnostic pins; no assumed voltage translation.
use conduit_tm1637_wire::OpenDrain;
use esp_hal::{
    delay::Delay,
    gpio::{Flex, InputConfig, Pin, Pull},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayPinError {
    ClockDidNotRise,
}
pub struct DisplayPins<'a> {
    clock: Flex<'a>,
    data: Flex<'a>,
    delay: Delay,
}
impl<'a> DisplayPins<'a> {
    pub fn new(clock: impl Pin + 'a, data: impl Pin + 'a) -> Self {
        let mut clock = Flex::new(clock);
        let mut data = Flex::new(data);
        for pin in [&mut clock, &mut data] {
            pin.set_output_enable(false);
            pin.set_low();
            pin.set_input_enable(true);
            pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
        }
        Self {
            clock,
            data,
            delay: Delay::new(),
        }
    }
    #[cfg(not(feature = "ap"))]
    pub fn check_drive(&mut self) {
        self.clock.set_output_enable(true);
        self.data.set_output_enable(true);
        self.delay.delay_micros(100);
        let low = self.levels();
        self.clock.set_output_enable(false);
        self.delay.delay_micros(100);
        self.data.set_output_enable(false);
        self.delay.delay_micros(100);
        let released = self.levels();
        esp_println::println!("{{\"event\":\"display-electrical-check\",\"driven_clock_high\":{},\"driven_data_high\":{},\"released_clock_high\":{},\"released_data_high\":{},\"internal_pullups\":true}}",low.0,low.1,released.0,released.1);
    }
    #[cfg(not(feature = "ap"))]
    pub fn levels(&self) -> (bool, bool) {
        (self.clock.is_high(), self.data.is_high())
    }
}
impl OpenDrain for DisplayPins<'_> {
    type Error = DisplayPinError;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        self.clock.set_output_enable(!released);
        if released {
            // Count the high half-period only after the released physical line
            // has risen. A finite refusal replaces an assumed clock edge.
            for _ in 0..1000 {
                if self.clock.is_high() {
                    return Ok(());
                }
                self.delay.delay_micros(1);
            }
            return Err(DisplayPinError::ClockDidNotRise);
        }
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
        self.delay.delay_micros(100);
        Ok(())
    }
}
impl Drop for DisplayPins<'_> {
    fn drop(&mut self) {
        self.clock.set_output_enable(false);
        self.data.set_output_enable(false);
    }
}
