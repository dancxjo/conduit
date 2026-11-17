//! Mutable, bounded decoding of already acquired current-condition responses.
//! Preparation allocates; refresh and projection use only admitted storage.
use super::*;
use conduit_core::{
    PreparedStructuredComposer as Composer, validate_canonical_structured_value as canonical,
};

use super::wire::Payload;
mod json;
use json::decode;

const LIMIT: usize = 4096;
type Result<T> = core::result::Result<T, AmbientWeatherRefusal>;
fn contract<T>(_: T) -> AmbientWeatherRefusal {
    AmbientWeatherRefusal::NativeContract
}
fn malformed<T>(_: T) -> AmbientWeatherRefusal {
    AmbientWeatherRefusal::MalformedResponse
}

#[derive(Clone, Copy)]
enum Field {
    Location,
    Attribution,
    Conditions,
    Temperature,
    Observed,
    Expires,
}
struct Slot {
    field: Field,
    composer: Composer,
}
/// No acquisition, clock sampling, scheduling or authority. The caller supplies
/// response bytes and each reference time. Rejected refreshes preserve custody
/// of the previous valid reading; projections always recompute freshness.
pub struct PreparedCurrentWeatherWire {
    location: Box<str>,
    maximum_age_ms: u64,
    slots: [Slot; 6],
    record: Composer,
    retained: Box<[u8]>,
    retained_len: usize,
    observed: u64,
    expires: u64,
    fresh: Payload,
    stale: Payload,
    weather: Composer,
    unavailable: Box<[u8]>,
    future: Box<[u8]>,
}
impl PreparedCurrentWeatherWire {
    pub fn prepare(location: &str, maximum_age_ms: u64) -> Result<Self> {
        let provider = PreparedOpenMeteoWeather::prepare(location, maximum_age_ms)?;
        let StructuredInfoTypeShape::Record { fields, .. } = provider.reading.value_type.shape()
        else {
            return Err(contract(()));
        };
        let slots = fields
            .iter()
            .map(|f| {
                let field = match f.name() {
                    "location" => Field::Location,
                    "attribution" => Field::Attribution,
                    "conditions" => Field::Conditions,
                    "temperature_celsius_milli" => Field::Temperature,
                    "observed_at_unix_ms" => Field::Observed,
                    "usable_until_unix_ms" => Field::Expires,
                    _ => return Err(contract(())),
                };
                Ok(Slot {
                    field,
                    composer: Composer::new(f.value_type(), LIMIT).map_err(contract)?,
                })
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(contract)?;
        Ok(Self {
            location: location.into(),
            maximum_age_ms,
            slots,
            record: Composer::new(&provider.reading.value_type, LIMIT).map_err(contract)?,
            retained: vec![0; LIMIT].into_boxed_slice(),
            retained_len: 0,
            observed: 0,
            expires: 0,
            fresh: Payload::prepare(&provider.fresh.value_type)?,
            stale: Payload::prepare(&provider.stale.value_type)?,
            weather: Composer::new(&provider.weather.value_type, LIMIT).map_err(contract)?,
            unavailable: provider
                .unavailable("Weather provider unavailable")?
                .canonical_bytes()
                .map_err(contract)?
                .into_boxed_slice(),
            future: provider
                .unavailable("Clock precedes weather validity")?
                .canonical_bytes()
                .map_err(contract)?
                .into_boxed_slice(),
        })
    }
    /// Complete owned heap backing; fixed inline state is reported separately.
    pub fn prepared_heap_bytes(&self) -> usize {
        self.location.len()
            + self.retained.len()
            + self.unavailable.len()
            + self.future.len()
            + self.record.prepared_heap_bytes()
            + self.weather.prepared_heap_bytes()
            + self.fresh.heap_bytes()
            + self.stale.heap_bytes()
            + self
                .slots
                .iter()
                .map(|s| s.composer.prepared_heap_bytes())
                .sum::<usize>()
    }
    pub const fn inline_bytes() -> usize {
        core::mem::size_of::<Self>()
    }
    pub fn refresh(&mut self, body: &[u8], now: u64) -> Result<()> {
        if body.len() > LIMIT {
            return Err(AmbientWeatherRefusal::ResponseTooLarge);
        }
        let parsed = decode(body)?;
        let observed = parsed
            .time
            .checked_mul(1000)
            .ok_or(AmbientWeatherRefusal::TimestampOverflow)?;
        let expires = observed
            .checked_add(self.maximum_age_ms)
            .ok_or(AmbientWeatherRefusal::TimestampOverflow)?;
        if observed > now {
            return Err(AmbientWeatherRefusal::FutureValidTime);
        }
        let conditions = condition(parsed.code)?;
        if !(-100_000..=100_000).contains(&parsed.temperature) {
            return Err(contract(()));
        }
        let temperature = parsed.temperature.to_le_bytes();
        let observed_bytes = observed.to_le_bytes();
        let expires_bytes = expires.to_le_bytes();
        let [a, b, c, d, e, f] = self.slots.each_mut().map(|slot| {
            let bytes = match slot.field {
                Field::Location => self.location.as_bytes(),
                Field::Attribution => ATTRIBUTION.as_bytes(),
                Field::Conditions => conditions.as_bytes(),
                Field::Temperature => &temperature,
                Field::Observed => &observed_bytes,
                Field::Expires => &expires_bytes,
            };
            slot.composer
                .leaf(bytes)
                .map_err(contract)
                .and_then(|b| canonical(b).map_err(contract))
        });
        let encoded = self
            .record
            .record(&[a?, b?, c?, d?, e?, f?])
            .map_err(contract)?;
        self.retained[..encoded.len()].copy_from_slice(encoded);
        self.retained_len = encoded.len();
        self.observed = observed;
        self.expires = expires;
        Ok(())
    }
    pub fn encode(&mut self, now: u64) -> Result<&[u8]> {
        if self.retained_len == 0 {
            return Ok(&self.unavailable);
        }
        if now < self.observed {
            return Ok(&self.future);
        }
        let (tag, payload) = if now <= self.expires {
            ("fresh", &mut self.fresh)
        } else {
            ("stale", &mut self.stale)
        };
        let bytes = payload.encode(&self.retained[..self.retained_len], now)?;
        self.weather
            .variant(tag, canonical(bytes).map_err(contract)?)
            .map_err(contract)
    }
}

#[cfg(test)]
#[path = "current_wire/tests.rs"]
mod tests;
