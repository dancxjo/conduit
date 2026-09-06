//! Ordered pulse observations carry an authored nominal period, not a measured clock.
use crate::{PulseObservation, MAXIMUM_PERIOD_MS, MINIMUM_PERIOD_MS};
use conduit_core::{ConfigurationEntry, ConfigurationValue};

pub const MAXIMUM_OBSERVED_PULSES: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseObservationConfiguration {
    pub period_ms: u16,
    pub maximum_pulses: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseObservationRefusal {
    Configuration,
    Exhausted,
    UnexpectedSequence { expected: u32, actual: u64 },
}

impl PulseObservationConfiguration {
    pub fn parse(entries: &[ConfigurationEntry]) -> Result<Self, PulseObservationRefusal> {
        let mut period_ms = None;
        let mut maximum_pulses = None;
        for entry in entries {
            let ConfigurationValue::U64(value) = entry.value else {
                return Err(PulseObservationRefusal::Configuration);
            };
            match entry.key.as_str() {
                "period-ms" if period_ms.is_none() => period_ms = u16::try_from(value).ok(),
                "maximum-pulses" if maximum_pulses.is_none() => {
                    maximum_pulses = u16::try_from(value).ok()
                }
                _ => return Err(PulseObservationRefusal::Configuration),
            }
        }
        let configuration = Self {
            period_ms: period_ms.ok_or(PulseObservationRefusal::Configuration)?,
            maximum_pulses: maximum_pulses.ok_or(PulseObservationRefusal::Configuration)?,
        };
        if entries.len() != 2
            || !(MINIMUM_PERIOD_MS..=MAXIMUM_PERIOD_MS).contains(&configuration.period_ms)
            || !(1..=MAXIMUM_OBSERVED_PULSES).contains(&configuration.maximum_pulses)
        {
            return Err(PulseObservationRefusal::Configuration);
        }
        Ok(configuration)
    }

    /// One ordered transition for each tick, beginning at zero. No coalescing,
    /// clock sampling, implicit sequence wrap, or replacement of missing ticks.
    pub fn observe(
        self,
        expected: u32,
        actual: u64,
    ) -> Result<PulseObservation, PulseObservationRefusal> {
        if !(MINIMUM_PERIOD_MS..=MAXIMUM_PERIOD_MS).contains(&self.period_ms)
            || !(1..=MAXIMUM_OBSERVED_PULSES).contains(&self.maximum_pulses)
        {
            return Err(PulseObservationRefusal::Configuration);
        }
        if expected >= u32::from(self.maximum_pulses) {
            return Err(PulseObservationRefusal::Exhausted);
        }
        if actual != u64::from(expected) {
            return Err(PulseObservationRefusal::UnexpectedSequence { expected, actual });
        }
        Ok(PulseObservation {
            sequence: expected,
            period_ms: self.period_ms,
        })
    }
}
