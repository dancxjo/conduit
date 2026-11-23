//! Typed snapshot construction during separately admitted provider work.
use super::{AmbientClockRefusal, PreparedAmbientClock};
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::{rust_binding, CheckedNativeType};

pub struct PreparedAmbientClockWeather {
    clock: PreparedAmbientClock,
    snapshot: CheckedNativeType,
}

impl PreparedAmbientClockWeather {
    pub fn prepare(timezone: &str) -> Result<Self, AmbientClockRefusal> {
        let checked = conduit_semantic_catalog::prepare_ambient_display_types()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let snapshot = checked
            .native_types
            .into_iter()
            .find(|ty| ty.name == "AmbientClockWeather")
            .ok_or(AmbientClockRefusal::NativeContract)?;
        Ok(Self {
            clock: PreparedAmbientClock::prepare(timezone)?,
            snapshot,
        })
    }

    /// Allocates a bounded typed value. Its storage and mandatory work must be
    /// admitted by the caller before use in a Host Call; this is not a Back.
    pub fn snapshot(
        &self,
        unix_millis: u64,
        weather: &StructuredInfoValue,
    ) -> Result<StructuredInfoValue, AmbientClockRefusal> {
        let reading = self.clock.project(unix_millis)?;
        let StructuredInfoTypeShape::Record { fields, .. } = self.snapshot.value_type.shape()
        else {
            return Err(AmbientClockRefusal::NativeContract);
        };
        let mut values = Vec::with_capacity(fields.len());
        for field in fields {
            let ty = field.value_type().clone();
            let value = match field.name() {
                "local_date" => {
                    rust_binding::primitive_into_structured(ty, &reading.local_date().to_owned())
                }
                "local_time" => {
                    rust_binding::primitive_into_structured(ty, &reading.local_time().to_owned())
                }
                "timezone" => {
                    rust_binding::primitive_into_structured(ty, &reading.timezone.to_owned())
                }
                "timezone_rule_set" => {
                    rust_binding::primitive_into_structured(ty, &reading.rule_set.to_owned())
                }
                "utc_offset_minutes" => {
                    rust_binding::primitive_into_structured(ty, &reading.utc_offset_minutes)
                }
                "observed_at_unix_ms" => rust_binding::primitive_into_structured(ty, &unix_millis),
                "weather" if weather.value_type() == &ty => Ok(weather.clone()),
                _ => return Err(AmbientClockRefusal::NativeContract),
            }
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
            values.push(
                StructuredFieldValue::new(field.name(), value)
                    .map_err(|_| AmbientClockRefusal::NativeContract)?,
            );
        }
        let value = StructuredInfoValue::record(self.snapshot.value_type.clone(), values)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        rust_binding::validate_native_contracts(&value, &self.snapshot.value_contracts)
            .and_then(|()| {
                rust_binding::validate_native_invariants(&value, &self.snapshot.invariants)
            })
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ambient_weather::PreparedOpenMeteoWeather;

    #[test]
    fn unavailable_weather_does_not_prevent_a_current_clock_snapshot() {
        let clock = PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
        let provider = PreparedOpenMeteoWeather::prepare("Seattle", 900_000).unwrap();
        let weather = provider.unavailable("Provider unavailable").unwrap();
        for now in [0, 1_794_805_200_000] {
            let value = clock.snapshot(now, &weather).unwrap();
            assert!(value.canonical_bytes().unwrap().len() <= 4096);
        }
    }

    #[test]
    fn retained_weather_must_be_reclassified_for_each_clock_observation() {
        let clock = PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
        let provider = PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap();
        let reading = provider
            .decode_reading(
                r#"{
                "current_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},
                "current":{"time":100,"temperature_2m":12.5,"weather_code":0}
            }"#
                .as_bytes(),
            )
            .unwrap();
        let fresh = provider.classify(&reading, 105_000).unwrap();
        clock.snapshot(105_000, &fresh).unwrap();
        assert_eq!(
            clock.snapshot(105_001, &fresh),
            Err(AmbientClockRefusal::NativeContract)
        );
        let stale = provider.classify(&reading, 105_001).unwrap();
        clock.snapshot(105_001, &stale).unwrap();
        assert_eq!(
            clock.snapshot(105_002, &stale),
            Err(AmbientClockRefusal::NativeContract)
        );
        // A failed refresh still permits the next clock observation, with an
        // explicit absence of current conditions rather than fabricated data.
        let unavailable = provider.unavailable("Weather request timed out").unwrap();
        clock.snapshot(105_002, &unavailable).unwrap();
    }
}
