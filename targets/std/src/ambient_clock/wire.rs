//! Prepared canonical encoder for clock reads; no growing typed-value tree.
use super::{AmbientClockReading, AmbientClockRefusal, PreparedAmbientClock};
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoTypeShape, ValidatedCanonicalStructuredValue,
    validate_canonical_structured_value as validate,
};

const MAXIMUM_BYTES: usize = 4096;

#[derive(Clone, Copy)]
enum Field {
    Date,
    Time,
    Timezone,
    Rules,
    Offset,
    Observed,
    Weather,
}
struct Slot {
    field: Field,
    composer: Option<PreparedStructuredComposer>,
}

/// Projects unavailable weather or one prepared provider reading, reclassifying
/// its freshness against every clock observation without growing storage.
pub struct PreparedAmbientClockWire {
    clock: PreparedAmbientClock,
    record: PreparedStructuredComposer,
    slots: [Slot; 7],
    weather: Box<[u8]>,
    cached_weather: Option<Box<crate::ambient_weather::PreparedAmbientWeatherWire>>,
}

impl PreparedAmbientClockWire {
    pub fn prepare(timezone: &str, unavailable_reason: &str) -> Result<Self, AmbientClockRefusal> {
        let clock = PreparedAmbientClock::prepare(timezone)?;
        let weather = crate::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 900_000)
            .map_err(|_| AmbientClockRefusal::NativeContract)?
            .unavailable(unavailable_reason)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let types = conduit_semantic_catalog::prepare_ambient_display_types()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let ty = &types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientClockWeather")
            .ok_or(AmbientClockRefusal::NativeContract)?
            .value_type;
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err(AmbientClockRefusal::NativeContract);
        };
        let weather_bytes = weather
            .canonical_bytes()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let weather_node_bytes = weather_bytes.len()
            - weather
                .value_type()
                .canonical_bytes()
                .map_err(|_| AmbientClockRefusal::NativeContract)?
                .len();
        let mut record_bytes = ty
            .canonical_bytes()
            .map_err(|_| AmbientClockRefusal::NativeContract)?
            .len()
            + 5;
        let slots = fields
            .iter()
            .map(|field| {
                let kind = match field.name() {
                    "local_date" => Field::Date,
                    "local_time" => Field::Time,
                    "timezone" => Field::Timezone,
                    "timezone_rule_set" => Field::Rules,
                    "utc_offset_minutes" => Field::Offset,
                    "observed_at_unix_ms" => Field::Observed,
                    "weather" => Field::Weather,
                    _ => return Err(AmbientClockRefusal::NativeContract),
                };
                let node_bytes = match kind {
                    Field::Date => 5 + 10,
                    Field::Time => 5 + 8,
                    Field::Timezone => 5 + clock.timezone.name().len(),
                    Field::Rules => 5 + chrono_tz::IANA_TZDB_VERSION.len(),
                    Field::Offset | Field::Observed => 5 + 8,
                    Field::Weather => weather_node_bytes,
                };
                record_bytes += 4 + field.name().len() + node_bytes;
                let leaf_bytes = field
                    .value_type()
                    .canonical_bytes()
                    .map_err(|_| AmbientClockRefusal::NativeContract)?
                    .len()
                    + node_bytes;
                let composer = if matches!(kind, Field::Weather) {
                    None
                } else {
                    Some(
                        PreparedStructuredComposer::new(field.value_type(), leaf_bytes)
                            .map_err(|_| AmbientClockRefusal::NativeContract)?,
                    )
                };
                Ok(Slot {
                    field: kind,
                    composer,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        if record_bytes > MAXIMUM_BYTES {
            return Err(AmbientClockRefusal::NativeContract);
        }
        Ok(Self {
            clock,
            record: PreparedStructuredComposer::new(ty, record_bytes)
                .map_err(|_| AmbientClockRefusal::NativeContract)?,
            slots,
            weather: weather_bytes.into_boxed_slice(),
            cached_weather: None,
        })
    }

    /// Prepare a clock with one checked provider reading. Each encode derives
    /// freshness against that exact clock observation; no verdict is replayed.
    /// Acquisition and replacement remain separately admitted provider work.
    pub fn prepare_with_weather_reading(
        timezone: &str,
        reading: &conduit_core::StructuredInfoValue,
    ) -> Result<Self, AmbientClockRefusal> {
        let mut clock = Self::prepare(timezone, "Weather provider unavailable")?;
        let cached = crate::ambient_weather::PreparedAmbientWeatherWire::prepare(reading)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let types = conduit_semantic_catalog::prepare_ambient_display_types()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let ty = types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientClockWeather")
            .ok_or(AmbientClockRefusal::NativeContract)?;
        clock.record = PreparedStructuredComposer::new(&ty.value_type, MAXIMUM_BYTES)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        clock.cached_weather = Some(Box::new(cached));
        Ok(clock)
    }

    /// Complete owned heap backing, excluding inline state/allocator overhead.
    pub fn prepared_heap_bytes(&self) -> usize {
        self.cached_weather.as_ref().map_or(0, |weather| {
            core::mem::size_of_val(weather.as_ref()) + weather.prepared_heap_bytes()
        }) + self.record.prepared_heap_bytes()
            + self.weather.len()
            + self
                .slots
                .iter()
                .filter_map(|slot| slot.composer.as_ref())
                .map(PreparedStructuredComposer::prepared_heap_bytes)
                .sum::<usize>()
    }

    /// Prepare the larger, fixed record envelope needed by live weather
    /// refreshes. The caller still admits acquisition and this complete heap.
    pub fn prepare_live_weather(timezone: &str) -> Result<Self, AmbientClockRefusal> {
        let mut wire = Self::prepare(timezone, "Weather not yet acquired")?;
        let types = conduit_semantic_catalog::prepare_ambient_display_types()
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        let ty = types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientClockWeather")
            .ok_or(AmbientClockRefusal::NativeContract)?;
        wire.record = PreparedStructuredComposer::new(&ty.value_type, MAXIMUM_BYTES)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        Ok(wire)
    }

    pub fn encode_with_current_weather(
        &mut self,
        unix_ms: u64,
        weather: &mut crate::ambient_weather::PreparedCurrentWeatherWire,
    ) -> Result<&[u8], AmbientClockRefusal> {
        let reading = self.clock.project(unix_ms)?;
        let weather = weather
            .encode(unix_ms)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        compose(
            &mut self.record,
            &mut self.slots,
            &reading,
            unix_ms,
            weather,
        )
    }

    pub fn encode(&mut self, unix_ms: u64) -> Result<&[u8], AmbientClockRefusal> {
        let reading = self.clock.project(unix_ms)?;
        let weather = match self.cached_weather.as_mut() {
            Some(weather) => weather
                .encode(unix_ms)
                .map_err(|_| AmbientClockRefusal::NativeContract)?,
            None => &self.weather,
        };
        compose(
            &mut self.record,
            &mut self.slots,
            &reading,
            unix_ms,
            weather,
        )
    }

    /// Invoke only within the caller's admitted platform-effect boundary.
    /// This samples the real wall clock; it does not schedule future reads.
    pub fn sample(&mut self) -> Result<&[u8], AmbientClockRefusal> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AmbientClockRefusal::TimestampOutOfRange)?;
        let millis = now
            .as_millis()
            .try_into()
            .map_err(|_| AmbientClockRefusal::TimestampOutOfRange)?;
        self.encode(millis)
    }
}

fn compose<'a>(
    record: &'a mut PreparedStructuredComposer,
    slots: &mut [Slot; 7],
    reading: &AmbientClockReading,
    unix_ms: u64,
    weather: &[u8],
) -> Result<&'a [u8], AmbientClockRefusal> {
    let offset = reading.utc_offset_minutes.to_le_bytes();
    let observed = unix_ms.to_le_bytes();
    let [a, b, c, d, e, f, g] = slots
        .each_mut()
        .map(|slot| slot.encode(reading, &offset, &observed, weather));
    record
        .record(&[a?, b?, c?, d?, e?, f?, g?])
        .map_err(|_| AmbientClockRefusal::NativeContract)
}

impl Slot {
    fn encode<'a>(
        &'a mut self,
        reading: &AmbientClockReading,
        offset: &[u8; 8],
        observed: &[u8; 8],
        weather: &'a [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, AmbientClockRefusal> {
        let bytes = match self.field {
            Field::Weather => {
                return validate(weather).map_err(|_| AmbientClockRefusal::NativeContract);
            }
            Field::Date => reading.local_date().as_bytes(),
            Field::Time => reading.local_time().as_bytes(),
            Field::Timezone => reading.timezone.as_bytes(),
            Field::Rules => reading.rule_set.as_bytes(),
            Field::Offset => offset,
            Field::Observed => observed,
        };
        let encoded = self
            .composer
            .as_mut()
            .ok_or(AmbientClockRefusal::NativeContract)?
            .leaf(bytes)
            .map_err(|_| AmbientClockRefusal::NativeContract)?;
        validate(encoded).map_err(|_| AmbientClockRefusal::NativeContract)
    }
}

impl PreparedAmbientClockWire {
    fn read_with_stop(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        should_stop: impl Fn() -> bool,
    ) -> Result<usize, crate::WorkerFailure> {
        use crate::WorkerFailure;
        if should_stop() {
            return Err(WorkerFailure::Cancelled);
        }
        conduit_time::decode_tick(input).map_err(|_| WorkerFailure::Provider)?;
        let encoded = self.sample().map_err(|_| WorkerFailure::Provider)?;
        if should_stop() {
            return Err(WorkerFailure::Cancelled);
        }
        let destination = output
            .get_mut(..encoded.len())
            .ok_or(WorkerFailure::InvalidOutput)?;
        destination.copy_from_slice(encoded);
        Ok(encoded.len())
    }
}

// SAFETY: preparation owns every schema/output buffer. read only decodes an
// exact eight-byte Tick, samples SystemTime and projects compiled timezone
// rules into those fixed buffers. It neither allocates nor retains caller
// borrows. Cancellation/deadline are checked before sampling and publication.
// Installation still requires an independently authorized matching clock
// Source definition and measured backing; implementing this trait grants none.
unsafe impl crate::NativeSourceWorkerProvider for PreparedAmbientClockWire {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &crate::NativeReadContext<'_>,
    ) -> Result<usize, crate::WorkerFailure> {
        self.read_with_stop(input, output, || context.should_stop())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_weather_reclassifies_without_allocation_or_clock_failure() {
        let provider =
            crate::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap();
        let reading = provider
            .decode_reading(
                r#"{
            "current_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},
            "current":{"time":100,"temperature_2m":12.5,"weather_code":0}
        }"#
                .as_bytes(),
            )
            .unwrap();
        let mut wire =
            PreparedAmbientClockWire::prepare_with_weather_reading("America/Los_Angeles", &reading)
                .unwrap();
        let typed =
            super::super::PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
        let backing = wire.prepared_heap_bytes();
        for now in [99_999, 100_000, 105_000, 105_001, 200_000, 100_001] {
            let weather = if now < 100_000 {
                provider
                    .unavailable("Clock precedes weather validity")
                    .unwrap()
            } else {
                provider.classify(&reading, now).unwrap()
            };
            let expected = typed
                .snapshot(now, &weather)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let probe = crate::allocation_probe::begin();
            assert_eq!(wire.encode(now).unwrap(), expected);
            assert_eq!(probe.finish_statistics().requested_bytes, 0);
            assert_eq!(wire.prepared_heap_bytes(), backing);
        }
        // A same-shaped arbitrary value is not accepted as a checked reading.
        let unavailable = provider.unavailable("No reading").unwrap();
        assert!(
            PreparedAmbientClockWire::prepare_with_weather_reading(
                "America/Los_Angeles",
                &unavailable
            )
            .is_err()
        );
    }

    #[test]
    fn prepared_wire_matches_typed_snapshot_without_read_allocations() {
        let mut wire = PreparedAmbientClockWire::prepare(
            "America/Los_Angeles",
            "Weather provider unavailable",
        )
        .unwrap();
        let typed =
            super::super::PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
        let weather = crate::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 900_000)
            .unwrap()
            .unavailable("Weather provider unavailable")
            .unwrap();
        let backing = wire.prepared_heap_bytes();
        for now in [0, 1_794_805_200_000, 1_794_805_260_000, 1_794_110_400_000] {
            let expected = typed
                .snapshot(now, &weather)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let probe = crate::allocation_probe::begin();
            assert_eq!(wire.encode(now).unwrap(), expected);
            assert_eq!(probe.finish_statistics().requested_bytes, 0);
            assert_eq!(wire.prepared_heap_bytes(), backing);
        }
        let probe = crate::allocation_probe::begin();
        assert!(!wire.sample().unwrap().is_empty());
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
    }

    #[test]
    fn worker_read_is_bounded_and_suppresses_cancelled_or_late_publication() {
        use crate::WorkerFailure;
        let mut wire = PreparedAmbientClockWire::prepare(
            "America/Los_Angeles",
            "Weather provider unavailable",
        )
        .unwrap();
        let mut output = [0xA5; MAXIMUM_BYTES];
        let tick = conduit_time::encode_tick(0);
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let probe = crate::allocation_probe::begin();
        let bytes = wire.read_with_stop(&tick, &mut output, || false).unwrap();
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let sample = validate(&output[..bytes]).unwrap();
        let observed = sample
            .record_field("observed_at_unix_ms")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap();
        let observed = u64::from_le_bytes(observed.try_into().unwrap()) as u128;
        assert!((before..=after).contains(&observed));
        output.fill(0xA5);
        assert_eq!(
            wire.read_with_stop(&tick, &mut output, || true),
            Err(WorkerFailure::Cancelled)
        );
        let calls = core::cell::Cell::new(0);
        assert_eq!(
            wire.read_with_stop(&tick, &mut output, || {
                calls.set(calls.get() + 1);
                calls.get() == 2
            }),
            Err(WorkerFailure::Cancelled)
        );
        assert!(output.iter().all(|byte| *byte == 0xA5));
        assert_eq!(
            wire.read_with_stop(b"", &mut output, || false),
            Err(WorkerFailure::Provider)
        );
        assert_eq!(
            wire.read_with_stop(&tick, &mut [], || false),
            Err(WorkerFailure::InvalidOutput)
        );
    }
}

#[cfg(test)]
#[path = "wire/live_weather_tests.rs"]
mod live_weather_tests;
