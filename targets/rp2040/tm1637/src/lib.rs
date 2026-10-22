#![no_std]
//! Selected RP2040 electrical provider. Installation must bind this owned pin
//! pair to an admitted Host Call; constructing it grants no Plan authority.
use conduit_tm1637_wire::OpenDrain;
use core::convert::Infallible;
use embassy_rp::{
    gpio::{Flex, Pin, Pull},
    Peri,
};
use embassy_time::{block_for, Duration};

pub struct PicoTm1637<'d> {
    clock: Flex<'d>,
    data: Flex<'d>,
}

impl<'d> PicoTm1637<'d> {
    /// Observed digital levels, not a voltage measurement or wiring proof.
    pub fn levels(&self) -> (bool, bool) {
        (self.clock.is_high(), self.data.is_high())
    }
    /// Consumes the selected pin ownership. The environment profile selects
    /// GP3 for CLK and GP4 for DIO. External pull-ups and a suitable 3.3V/5V
    /// bidirectional level shifter are required for the user's 5V display.
    pub fn new(clock: Peri<'d, impl Pin>, data: Peri<'d, impl Pin>) -> Self {
        Self {
            clock: released_pin(clock),
            data: released_pin(data),
        }
    }
}

fn released_pin<'d>(pin: Peri<'d, impl Pin>) -> Flex<'d> {
    let mut pin = Flex::new(pin);
    pin.set_as_input();
    pin.set_pull(Pull::None);
    pin.set_low(); // Preload the output latch; never drive high.
    pin
}

fn drive(pin: &mut Flex<'_>, released: bool) {
    if released {
        pin.set_as_input();
    } else {
        pin.set_as_output();
    }
}

impl OpenDrain for PicoTm1637<'_> {
    type Error = Infallible;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        drive(&mut self.clock, released);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), Self::Error> {
        drive(&mut self.data, released);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(self.data.is_high())
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        block_for(Duration::from_micros(5));
        Ok(())
    }
}

impl Drop for PicoTm1637<'_> {
    fn drop(&mut self) {
        self.data.set_as_input();
        self.clock.set_as_input();
    }
}
