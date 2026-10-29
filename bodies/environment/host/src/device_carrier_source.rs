//! Environmental interpretation above the generic owned read-only USB carrier.
//! Board differences come from the independently checked Host configuration.
//! Native observer credentials are not a grant to install or invoke this Base.
use conduit_core::{HostAdvertisement, Quantity, Unit};
use conduit_environment_runtime::environmental_publication::{
    SourceFailure, SourceOutcome, SourceSample,
};
use conduit_environment_runtime::environmental_reading::ReadingField;
use conduit_environment_runtime::{
    environmental_reading::EnvironmentalReading,
    environmental_source::{EnvironmentalSourceProvider, Refusal},
};
use conduit_host_make::{CheckedHostConfiguration, ConfigurationDevice};
use conduit_std_host::device_carrier::{DeviceCarrier, ExpectedDeviceIdentity, ReadOnlyQuery};
use serde::Deserialize;
use std::{
    path::Path,
    time::{Duration, Instant},
};
// Keep the common compact observation contract outside std's dependency graph.
use conduit_environment_runtime::environmental_reading::source_observation::{
    SourceContinuity, SourceContinuityRefusal, SourceObservation,
};
#[derive(Clone, Copy)]
enum Encoding {
    Bosch,
    Dht11,
}
pub struct DeviceEnvironmentalSource<'a> {
    carrier: DeviceCarrier,
    host: &'a HostAdvertisement,
    encoding: Encoding,
    continuity: SourceContinuity,
    acquisition_budget: Duration,
    query_budget: Duration,
    failed: bool,
}
impl<'a> DeviceEnvironmentalSource<'a> {
    /// Initialize once and retain this same owner (including continuity) across
    /// child Plays. Root must separately advertise/admit its finite session,
    /// bounded worker time and exact Source Host Call before invocation. This
    /// synchronous native I/O belongs on that admitted worker, not a display loop.
    pub fn acquire(
        path: &Path,
        host: &'a HostAdvertisement,
        configuration: &CheckedHostConfiguration,
        firmware: [u8; 32],
        acquisition_budget: Duration,
    ) -> Result<Self, Refusal> {
        if acquisition_budget < Duration::from_secs(31)
            || acquisition_budget > Duration::from_secs(45)
        {
            return Err(Refusal::Possession);
        }
        let encoding = encoding(configuration)?;
        let digest = configuration
            .configuration_id()
            .strip_prefix("sha256:")
            .ok_or(Refusal::Possession)?;
        let configuration = parse_digest(digest)?;
        let query_budget = Duration::from_secs(5);
        let carrier = DeviceCarrier::acquire(
            path,
            host,
            ExpectedDeviceIdentity {
                role: "conduit.environment-wifi-carrier@1".into(),
                firmware,
                configuration,
            },
            query_budget,
        )
        .map_err(|_| Refusal::Provider)?;
        Ok(Self {
            carrier,
            host,
            encoding,
            continuity: SourceContinuity::default(),
            acquisition_budget,
            query_budget,
            failed: false,
        })
    }
}
impl EnvironmentalSourceProvider for DeviceEnvironmentalSource<'_> {
    async fn observe(&mut self) -> Result<SourceObservation, Refusal> {
        if self.failed {
            return Err(Refusal::Provider);
        }
        let deadline = Instant::now()
            .checked_add(self.acquisition_budget)
            .ok_or(Refusal::Provider)?;
        let mut last_observed_at_host_boot_ms = None;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_default();
            if remaining.is_zero() {
                // A timeout is an observation about this bounded capture, not a
                // freshly acquired sensor value. Retain the last actual Host
                // observation time instead of manufacturing an acquisition time.
                return last_observed_at_host_boot_ms
                    .map(|observed_at_host_boot_ms| SourceObservation {
                        observed_at_host_boot_ms,
                        outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
                    })
                    .ok_or(Refusal::Provider);
            }
            let info = match self.carrier.query_with_budget(
                self.host,
                ReadOnlyQuery::SensorStatus,
                remaining.min(self.query_budget),
            ) {
                Ok(info) => info,
                Err(_) => {
                    self.failed = true;
                    return Err(Refusal::Provider);
                }
            };
            let observed_at_host_boot_ms = info.observed_at_host_boot_ms;
            last_observed_at_host_boot_ms = Some(observed_at_host_boot_ms);
            let outcome = match decode(info.payload, self.encoding) {
                Ok(outcome) => outcome,
                Err(_) => {
                    self.failed = true;
                    return Ok(SourceObservation {
                        observed_at_host_boot_ms,
                        outcome: SourceOutcome::Unavailable(SourceFailure::InvalidSample),
                    });
                }
            };
            match outcome {
                SourceOutcome::Unavailable(_) => {
                    return Ok(SourceObservation {
                        observed_at_host_boot_ms,
                        outcome,
                    })
                }
                SourceOutcome::Available(sample) => match self
                    .continuity
                    .observe(&sample, observed_at_host_boot_ms)
                {
                    Ok(()) => {
                        return Ok(SourceObservation {
                            observed_at_host_boot_ms,
                            outcome,
                        })
                    }
                    Err(SourceContinuityRefusal::NoNewObservation) => {
                        let remaining = deadline
                            .checked_duration_since(Instant::now())
                            .unwrap_or_default();
                        if remaining.is_zero() {
                            return Ok(SourceObservation {
                                observed_at_host_boot_ms,
                                outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
                            });
                        }
                        std::thread::sleep(remaining.min(Duration::from_millis(250)));
                    }
                    Err(_) => {
                        self.failed = true;
                        return Err(Refusal::Provider);
                    }
                },
            }
        }
    }
}
fn encoding(configuration: &CheckedHostConfiguration) -> Result<Encoding, Refusal> {
    let mut found = None;
    for device in &configuration.configuration().devices {
        let candidate = match device {
            ConfigurationDevice::I2c {
                id, implementation, ..
            } if id == "sensor" && implementation == "bosch-environment@1" => Some(Encoding::Bosch),
            ConfigurationDevice::Gpio {
                id, implementation, ..
            } if id == "sensor" && implementation == "dht11@1" => Some(Encoding::Dht11),
            _ => None,
        };
        if let Some(candidate) = candidate {
            if found.is_some() {
                return Err(Refusal::Possession);
            }
            found = Some(candidate);
        }
    }
    found.ok_or(Refusal::Possession)
}
fn parse_digest(text: &str) -> Result<[u8; 32], Refusal> {
    if text.len() != 64 {
        return Err(Refusal::Possession);
    }
    let mut bytes = [0; 32];
    for (out, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(Refusal::Possession),
        };
        *out = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(bytes)
}
#[derive(Deserialize)]
struct Decimal {
    coefficient: i128,
    exponent: i16,
}
#[derive(Deserialize)]
struct Common {
    body_member: bool,
    samples: u64,
    attempts: u64,
}
#[derive(Deserialize)]
struct Bosch {
    body_member: bool,
    samples: u64,
    attempts: u64,
    sensor_error: bool,
    temperature_centi_celsius: Option<i32>,
    pressure_centi_pascal: Option<u32>,
    #[serde(default, deserialize_with = "humidity_field")]
    humidity_percent_q10: HumidityField,
    sampled_at_boot_ms: Option<u64>,
}
#[derive(Deserialize)]
struct Dht<'a> {
    body_member: bool,
    samples: u64,
    attempts: u64,
    #[serde(borrow)]
    status: &'a str,
    temperature: Option<Decimal>,
    humidity: Option<Decimal>,
}
fn decode(bytes: &[u8], encoding: Encoding) -> Result<SourceOutcome, Refusal> {
    let (common, outcome) = match encoding {
        Encoding::Bosch => {
            let value: Bosch = serde_json::from_slice(bytes).map_err(|_| Refusal::Provider)?;
            let outcome = if value.sensor_error || value.samples == 0 {
                SourceOutcome::Unavailable(SourceFailure::ProviderFailed)
            } else {
                let reading = EnvironmentalReading::centi_celsius_centi_pascal(
                    value.temperature_centi_celsius.ok_or(Refusal::Provider)?,
                    value.pressure_centi_pascal.ok_or(Refusal::Provider)?,
                    match value.humidity_percent_q10 {
                        HumidityField::Present(value) => value,
                        HumidityField::Missing => return Err(Refusal::Provider),
                    },
                )
                .map_err(|_| Refusal::Provider)?;
                SourceOutcome::Available(SourceSample {
                    reading,
                    source_counter: value.samples,
                    reported_source_ms: value.sampled_at_boot_ms,
                })
            };
            (
                Common {
                    body_member: value.body_member,
                    samples: value.samples,
                    attempts: value.attempts,
                },
                outcome,
            )
        }
        Encoding::Dht11 => {
            let value: Dht<'_> = serde_json::from_slice(bytes).map_err(|_| Refusal::Provider)?;
            let outcome = if value.status != "ok" || value.samples == 0 {
                SourceOutcome::Unavailable(SourceFailure::ProviderFailed)
            } else {
                let temperature = value.temperature.ok_or(Refusal::Provider)?;
                let humidity = value.humidity.ok_or(Refusal::Provider)?;
                let reading = EnvironmentalReading::new(
                    Quantity::from_decimal(
                        temperature.coefficient,
                        temperature.exponent,
                        Unit::Celsius,
                    )
                    .map_err(|_| Refusal::Provider)?,
                    ReadingField::Available(
                        Quantity::from_decimal(
                            humidity.coefficient,
                            humidity.exponent,
                            Unit::Percent,
                        )
                        .map_err(|_| Refusal::Provider)?,
                    ),
                    ReadingField::Unsupported,
                )
                .map_err(|_| Refusal::Provider)?;
                SourceOutcome::Available(SourceSample {
                    reading,
                    source_counter: value.samples,
                    reported_source_ms: None,
                })
            };
            (
                Common {
                    body_member: value.body_member,
                    samples: value.samples,
                    attempts: value.attempts,
                },
                outcome,
            )
        }
    };
    if common.body_member || common.samples > common.attempts {
        return Err(Refusal::Provider);
    }
    Ok(outcome)
}
#[cfg(test)]
#[path = "device_carrier_source/tests.rs"]
mod tests;

#[derive(Default)]
enum HumidityField {
    #[default]
    Missing,
    Present(Option<u32>),
}
fn humidity_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<HumidityField, D::Error> {
    Ok(HumidityField::Present(Option::<u32>::deserialize(
        deserializer,
    )?))
}
