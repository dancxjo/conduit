//! Allocation-free BME280/BMP280 register driver, not a capability or Body offer.
//! Caller owns the actual I2C bus and must bound each operation/whole future with
//! its platform deadline. Poll counts here are finite; a stuck bus still needs
//! that external deadline. Cancellation invalidates readiness; initialize again.
#![no_std]
#[cfg(test)]
extern crate std;
mod calibration;
use calibration::Calibration;
use embedded_hal_async::{delay::DelayNs, i2c::I2c};

pub const MAXIMUM_STATUS_POLLS: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Primary = 0x76,
    Secondary = 0x77,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    Bme280,
    Bmp280,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumidityUnavailable {
    UnsupportedChip,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Humidity {
    /// Relative humidity in1024ths of one percent;0..=102400 means0..=100%RH.
    PercentQ10(u32),
    Unavailable(HumidityUnavailable),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub temperature_centi_celsius: i32,
    /// Hundredths of one Pascal, matching Bosch's integer64 SensorAPI output.
    pub pressure_centi_pascal: u32,
    pub humidity: Humidity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataRefusal {
    Calibration,
    SkippedData,
    Arithmetic,
    OutOfRange,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    UnsupportedChip(u8),
    BusyTimeout,
    NotInitialized,
    Data(DataRefusal),
}
/// Exclusive bus owner. Constructing this value establishes no sensor presence;
/// only successful initialize reads a supported ID/calibration and acknowledges
/// configuration. No execution authority is created by either operation.
pub struct Sensor<I> {
    bus: I,
    address: Address,
    ready: Option<(Chip, Calibration)>,
}
impl<I: I2c> Sensor<I> {
    pub fn new(bus: I, address: Address) -> Self {
        Self {
            bus,
            address,
            ready: None,
        }
    }
    pub fn chip(&self) -> Option<Chip> {
        self.ready.as_ref().map(|v| v.0)
    }
    pub fn address(&self) -> Address {
        self.address
    }
    pub fn into_inner(self) -> I {
        self.bus
    }
    pub async fn initialize(&mut self, delay: &mut impl DelayNs) -> Result<Chip, Error<I::Error>> {
        self.ready = None;
        let mut id = [0];
        self.read(0xd0, &mut id).await?;
        let chip = match id[0] {
            0x60 => Chip::Bme280,
            0x58 => Chip::Bmp280,
            other => return Err(Error::UnsupportedChip(other)),
        };
        self.write(0xe0, 0xb6).await?;
        delay.delay_us(2_000).await;
        self.wait_clear(0x01, delay).await?;
        let mut tp = [0; 26];
        let len = if chip == Chip::Bme280 { 26 } else { 24 };
        self.read(0x88, &mut tp[..len]).await?;
        let mut humidity = [0; 7];
        if chip == Chip::Bme280 {
            self.read(0xe1, &mut humidity).await?;
        }
        let calibration = Calibration::parse(chip, &tp, &humidity).map_err(Error::Data)?;
        self.write(0xf5, 0).await?; // sleep mode after reset: IIR disabled
        if chip == Chip::Bme280 {
            self.write(0xf2, 1).await?;
        }
        self.write(0xf4, 0x24).await?; // x1 temperature/pressure, sleep; latch humidity
        self.ready = Some((chip, calibration));
        Ok(chip)
    }
    pub async fn measure(&mut self, delay: &mut impl DelayNs) -> Result<Reading, Error<I::Error>> {
        // Taking readiness before the first await makes cancellation fail closed.
        let (chip, calibration) = self.ready.take().ok_or(Error::NotInitialized)?;
        if chip == Chip::Bme280 {
            self.write(0xf2, 1).await?;
        }
        self.write(0xf4, 0x25).await?; // forced mode; humidity must be written first
        delay.delay_us(10_000).await; // x1 T/P/H maximum9.3ms (BMP6.425ms)
        self.wait_clear(0x09, delay).await?;
        let mut raw = [0; 8];
        let len = if chip == Chip::Bme280 { 8 } else { 6 };
        self.read(0xf7, &mut raw[..len]).await?;
        let reading = calibration.compensate(chip, &raw).map_err(Error::Data)?;
        self.ready = Some((chip, calibration));
        Ok(reading)
    }
    async fn read(&mut self, register: u8, out: &mut [u8]) -> Result<(), Error<I::Error>> {
        self.bus
            .write_read(self.address as u8, &[register], out)
            .await
            .map_err(Error::Bus)
    }
    async fn write(&mut self, register: u8, value: u8) -> Result<(), Error<I::Error>> {
        self.bus
            .write(self.address as u8, &[register, value])
            .await
            .map_err(Error::Bus)
    }
    async fn wait_clear(
        &mut self,
        mask: u8,
        delay: &mut impl DelayNs,
    ) -> Result<(), Error<I::Error>> {
        for poll in 0..MAXIMUM_STATUS_POLLS {
            let mut status = [0];
            self.read(0xf3, &mut status).await?;
            if status[0] & mask == 0 {
                return Ok(());
            }
            if poll + 1 < MAXIMUM_STATUS_POLLS {
                delay.delay_us(2_000).await;
            }
        }
        Err(Error::BusyTimeout)
    }
}
