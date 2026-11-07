//! Exclusive GPIO adapter with checked finite timing; transport remains shared.
use crate::config;
use conduit_tm1637_wire::OpenDrain;
use embassy_rp::{
    gpio::{Flex, Pin, Pull},
    Peri,
};
use embassy_time::{block_for, Duration, Instant};
#[derive(Debug)]
pub enum Refusal {
    TransactionDeadline,
    ClockDidNotRise,
}
pub struct Pins {
    clock: Flex<'static>,
    data: Flex<'static>,
    started: Instant,
}
impl Pins {
    pub fn new(clock: Peri<'static, impl Pin>, data: Peri<'static, impl Pin>) -> Self {
        fn released(pin: Peri<'static, impl Pin>) -> Flex<'static> {
            let mut pin = Flex::new(pin);
            pin.set_as_input();
            pin.set_low();
            pin.set_pull(if config::TM_WEAK_PULLUPS {
                Pull::Up
            } else {
                Pull::None
            });
            pin
        }
        Self {
            clock: released(clock),
            data: released(data),
            started: Instant::now(),
        }
    }
    pub fn begin(&mut self) {
        self.started = Instant::now();
        self.clock.set_as_input();
        self.data.set_as_input();
        block_for(Duration::from_micros(u64::from(config::TM_SETTLED_US)));
    }
    fn check(&self) -> Result<(), Refusal> {
        if self.started.elapsed().as_micros() >= u64::from(config::TM_TRANSACTION_US) {
            Err(Refusal::TransactionDeadline)
        } else {
            Ok(())
        }
    }
}
impl OpenDrain for Pins {
    type Error = Refusal;
    fn begin_transaction(&mut self) -> Result<(), Refusal> {
        self.begin();
        Ok(())
    }
    fn clock(&mut self, released: bool) -> Result<(), Refusal> {
        if released {
            // Release even after refusal so shared transport cleanup is safe.
            self.clock.set_as_input();
            let start = Instant::now();
            loop {
                self.check()?;
                if self.clock.is_high() {
                    return Ok(());
                }
                if start.elapsed().as_micros() >= u64::from(config::TM_CLOCK_RISE_US) {
                    return Err(Refusal::ClockDidNotRise);
                }
                block_for(Duration::from_micros(1));
            }
        }
        self.check()?;
        self.clock.set_as_output();
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), Refusal> {
        if released {
            self.data.set_as_input();
            self.check()
        } else {
            self.check()?;
            self.data.set_as_output();
            Ok(())
        }
    }
    fn read_data(&mut self) -> Result<bool, Refusal> {
        self.check()?;
        Ok(self.data.is_high())
    }
    fn half_period(&mut self) -> Result<(), Refusal> {
        self.check()?;
        let end = self.started.elapsed().as_micros() + u64::from(config::TM_HALF_PERIOD_US);
        if end >= u64::from(config::TM_TRANSACTION_US) {
            return Err(Refusal::TransactionDeadline);
        }
        block_for(Duration::from_micros(u64::from(config::TM_HALF_PERIOD_US)));
        self.check()
    }
}
impl Drop for Pins {
    fn drop(&mut self) {
        self.clock.set_as_input();
        self.data.set_as_input();
    }
}
