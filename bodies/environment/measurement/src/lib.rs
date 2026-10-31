#![no_std]
//! Common successful environmental readings, independent of a sensor or target.
//!
//! These values describe measurements, not admitted capabilities, publication
//! authority, provenance, or a wire format. A failed read remains a failure in
//! its original protocol; `Unsupported` must never stand in for that failure.
//! Legacy DHT Forms and signed records are deliberately unchanged.
use conduit_core::{Quantity, Unit};

/// A field present in a successful reading, or unsupported by that sensor.
// Keep exact Quantity inline: this adapter must not allocate a Box per field.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingField {
    Available(Quantity),
    Unsupported,
}

/// Exact common quantities. Unsupported fields are never represented by zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentalReading {
    temperature: Quantity,
    relative_humidity: ReadingField,
    atmospheric_pressure: ReadingField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingRefusal {
    Temperature,
    RelativeHumidity,
    AtmosphericPressure,
}

impl EnvironmentalReading {
    /// Normalize already decoded quantities without changing their coordinates
    /// or reviewed units. The caller retains the original contract and scope.
    pub fn new(
        temperature: Quantity,
        relative_humidity: ReadingField,
        atmospheric_pressure: ReadingField,
    ) -> Result<Self, ReadingRefusal> {
        if temperature.family() != Unit::Celsius.family()
            || temperature.role() != Unit::Celsius.declared_role()
        {
            return Err(ReadingRefusal::Temperature);
        }
        if let ReadingField::Available(value) = relative_humidity {
            if value.family() != Unit::Percent.family()
                || value.role() != Unit::Percent.declared_role()
                || value
                    .compare(Quantity::new(0, Unit::Percent))
                    .map_err(|_| ReadingRefusal::RelativeHumidity)?
                    .is_lt()
                || value
                    .compare(Quantity::new(100, Unit::Percent))
                    .map_err(|_| ReadingRefusal::RelativeHumidity)?
                    .is_gt()
            {
                return Err(ReadingRefusal::RelativeHumidity);
            }
        }
        if let ReadingField::Available(value) = atmospheric_pressure {
            if value.family() != Unit::Pascal.family()
                || value.role() != Unit::Pascal.declared_role()
                || !value
                    .compare(Quantity::new(0, Unit::Pascal))
                    .map_err(|_| ReadingRefusal::AtmosphericPressure)?
                    .is_gt()
            {
                return Err(ReadingRefusal::AtmosphericPressure);
            }
        }
        Ok(Self {
            temperature,
            relative_humidity,
            atmospheric_pressure,
        })
    }

    /// Lift a successful legacy temperature/humidity observation. This does
    /// not reinterpret or re-sign its original Type, Forms, or record bytes.
    pub fn temperature_humidity(
        temperature: Quantity,
        humidity: Quantity,
    ) -> Result<Self, ReadingRefusal> {
        Self::new(
            temperature,
            ReadingField::Available(humidity),
            ReadingField::Unsupported,
        )
    }

    /// Adapter for centi-Celsius, centi-Pascal and optional Q10 percent.
    /// `None` means the chip does not support humidity (BMP280), not a failed
    /// BME280 measurement. Q10 conversion is exact, with no decimal rounding.
    /// This is not the older authored BME Q24.8 pressure representation.
    pub fn centi_celsius_centi_pascal(
        temperature: i32,
        pressure: u32,
        humidity_q10_percent: Option<u32>,
    ) -> Result<Self, ReadingRefusal> {
        let temperature = Quantity::from_decimal(i128::from(temperature), -2, Unit::Celsius)
            .map_err(|_| ReadingRefusal::Temperature)?;
        let pressure = Quantity::from_decimal(i128::from(pressure), -2, Unit::Pascal)
            .map_err(|_| ReadingRefusal::AtmosphericPressure)?;
        let humidity = match humidity_q10_percent {
            Some(value) => ReadingField::Available(
                Quantity::from_decimal(i128::from(value) * 9_765_625, -10, Unit::Percent)
                    .map_err(|_| ReadingRefusal::RelativeHumidity)?,
            ),
            None => ReadingField::Unsupported,
        };
        Self::new(temperature, humidity, ReadingField::Available(pressure))
    }

    pub const fn temperature(self) -> Quantity {
        self.temperature
    }
    pub const fn relative_humidity(self) -> ReadingField {
        self.relative_humidity
    }
    pub const fn atmospheric_pressure(self) -> ReadingField {
        self.atmospheric_pressure
    }
}

pub mod source_observation;
