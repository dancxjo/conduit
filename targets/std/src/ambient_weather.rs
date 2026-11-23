//! Finite decoding of already acquired Open-Meteo current-condition responses.
//!
//! Preparation and decoding allocate. This is not a Play Back, network provider,
//! timer or authority check. A caller must separately admit the exchange, its
//! response storage and this work, and correlate the response with its request.
//! Current conditions are model estimates, not physical sensor observations.
//! Protocol reference: <https://open-meteo.com/en/docs>.
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{rust_binding, CheckedNativeType};
use serde::Deserialize;

mod current_wire;
mod decimal;
mod wire;
pub use current_wire::PreparedCurrentWeatherWire;
pub use wire::PreparedAmbientWeatherWire;

pub const MAXIMUM_OPEN_METEO_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAXIMUM_WEATHER_AGE_MS: u64 = 3_600_000;
const ATTRIBUTION: &str = "Open-Meteo modeled current conditions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientWeatherRefusal {
    InvalidPolicy,
    ResponseTooLarge,
    MalformedResponse,
    UnsupportedUnits,
    UnknownWeatherCode(u16),
    TemperaturePrecision,
    TimestampOverflow,
    FutureValidTime,
    WrongReading,
    NativeContract,
    Preparation(String),
}

/// Checked source contracts retained once during preparation. `maximum_age_ms`
/// is an explicit local freshness policy. The provider's `current.interval`
/// describes aggregation over past time; it must never become an expiry.
pub struct PreparedOpenMeteoWeather {
    location: String,
    maximum_age_ms: u64,
    reading: CheckedNativeType,
    fresh: CheckedNativeType,
    stale: CheckedNativeType,
    weather: CheckedNativeType,
}

impl PreparedOpenMeteoWeather {
    pub fn prepare(location: &str, maximum_age_ms: u64) -> Result<Self, AmbientWeatherRefusal> {
        if location.is_empty()
            || location.len() > 80
            || maximum_age_ms == 0
            || maximum_age_ms > MAXIMUM_WEATHER_AGE_MS
        {
            return Err(AmbientWeatherRefusal::InvalidPolicy);
        }
        let checked = conduit_semantic_catalog::prepare_ambient_display_types()
            .map_err(AmbientWeatherRefusal::Preparation)?;
        let native = |name| {
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .cloned()
                .ok_or_else(|| AmbientWeatherRefusal::Preparation(format!("missing {name}")))
        };
        Ok(Self {
            location: location.into(),
            maximum_age_ms,
            reading: native("AmbientWeatherReading")?,
            fresh: native("AmbientFreshWeather")?,
            stale: native("AmbientStaleWeather")?,
            weather: native("AmbientWeather")?,
        })
    }

    /// Decode one exact Celsius/Unix-UTC response. The configured location is a
    /// request binding supplied by the caller; returned grid coordinates are not
    /// an independent identity or evidence that this request was authorized.
    pub fn decode_reading(
        &self,
        body: &[u8],
    ) -> Result<StructuredInfoValue, AmbientWeatherRefusal> {
        if body.len() > MAXIMUM_OPEN_METEO_RESPONSE_BYTES {
            return Err(AmbientWeatherRefusal::ResponseTooLarge);
        }
        let response: Response =
            serde_json::from_slice(body).map_err(|_| AmbientWeatherRefusal::MalformedResponse)?;
        if response.current_units.time != "unixtime"
            || response.current_units.temperature_2m != "°C"
            || response.current_units.weather_code != "wmo code"
        {
            return Err(AmbientWeatherRefusal::UnsupportedUnits);
        }
        let observed = response
            .current
            .time
            .checked_mul(1000)
            .ok_or(AmbientWeatherRefusal::TimestampOverflow)?;
        let expires = observed
            .checked_add(self.maximum_age_ms)
            .ok_or(AmbientWeatherRefusal::TimestampOverflow)?;
        let temperature = decimal::temperature_millidegrees(body)?;
        let conditions = condition(response.current.weather_code)?;
        let value = record(&self.reading.value_type, |name, ty| match name {
            "location" => primitive(ty, self.location.clone()),
            "attribution" => primitive(ty, ATTRIBUTION.to_owned()),
            "conditions" => primitive(ty, conditions.to_owned()),
            "temperature_celsius_milli" => primitive(ty, temperature),
            "observed_at_unix_ms" => primitive(ty, observed),
            "usable_until_unix_ms" => primitive(ty, expires),
            _ => Err(AmbientWeatherRefusal::NativeContract),
        })?;
        validate(&self.reading, &value)?;
        Ok(value)
    }

    /// Reclassify the same retained reading on each caller clock observation.
    /// Expiry is inclusive, matching the authored fresh/stale laws. A future
    /// provider validity timestamp is rejected rather than disguised as fresh.
    pub fn classify(
        &self,
        reading: &StructuredInfoValue,
        reference_at_unix_ms: u64,
    ) -> Result<StructuredInfoValue, AmbientWeatherRefusal> {
        validate(&self.reading, reading).map_err(|_| AmbientWeatherRefusal::WrongReading)?;
        let observed: u64 =
            rust_binding::primitive_from_structured(field(reading, "observed_at_unix_ms")?)
                .map_err(|_| AmbientWeatherRefusal::WrongReading)?;
        let expires: u64 =
            rust_binding::primitive_from_structured(field(reading, "usable_until_unix_ms")?)
                .map_err(|_| AmbientWeatherRefusal::WrongReading)?;
        if reference_at_unix_ms < observed {
            return Err(AmbientWeatherRefusal::FutureValidTime);
        }
        let (tag, ty) = if reference_at_unix_ms <= expires {
            ("fresh", &self.fresh)
        } else {
            ("stale", &self.stale)
        };
        let payload = record(&ty.value_type, |name, field_type| match name {
            "reading" => Ok(reading.clone()),
            "reference_at_unix_ms" => primitive(field_type, reference_at_unix_ms),
            _ => Err(AmbientWeatherRefusal::NativeContract),
        })?;
        validate(ty, &payload)?;
        let value = StructuredInfoValue::variant(self.weather.value_type.clone(), tag, payload)
            .map_err(|_| AmbientWeatherRefusal::NativeContract)?;
        validate(&self.weather, &value)?;
        Ok(value)
    }

    /// A caller can project a bounded explicit failure without supplying fake
    /// temperature or condition data. Its reason must obey the authored bound.
    pub fn unavailable(&self, reason: &str) -> Result<StructuredInfoValue, AmbientWeatherRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = self.weather.value_type.shape() else {
            return Err(AmbientWeatherRefusal::NativeContract);
        };
        let case = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or(AmbientWeatherRefusal::NativeContract)?;
        let value = StructuredInfoValue::variant(
            self.weather.value_type.clone(),
            "unavailable",
            primitive(case.payload_type(), reason.to_owned())?,
        )
        .map_err(|_| AmbientWeatherRefusal::NativeContract)?;
        validate(&self.weather, &value)?;
        Ok(value)
    }
}

#[derive(Deserialize)]
struct Response {
    current_units: Units,
    current: Current,
}

#[derive(Deserialize)]
struct Units {
    time: String,
    temperature_2m: String,
    weather_code: String,
}

#[derive(Deserialize)]
struct Current {
    time: u64,
    #[serde(rename = "temperature_2m")]
    _temperature_2m: serde_json::Number,
    weather_code: u16,
}

fn condition(code: u16) -> Result<&'static str, AmbientWeatherRefusal> {
    Ok(match code {
        0 => "Clear",
        1 => "Mostly clear",
        2 => "Some clouds",
        3 => "Cloudy",
        45 => "Foggy",
        48 => "Fog with rime",
        51 => "Drizzle, light",
        53 => "Drizzle, moderate",
        55 => "Drizzle, dense",
        56 => "Freezing drizzle, light",
        57 => "Freezing drizzle, dense",
        61 => "Rain, light",
        63 => "Rain, moderate",
        65 => "Rain, heavy",
        66 => "Freezing rain, light",
        67 => "Freezing rain, heavy",
        71 => "Snow, light",
        73 => "Snow, moderate",
        75 => "Snow, heavy",
        77 => "Grains of snow",
        80 => "Rain showers, light",
        81 => "Rain showers, moderate",
        82 => "Rain showers, severe",
        85 => "Snow showers, light",
        86 => "Snow showers, heavy",
        95 => "Thunderstorms",
        96 => "Thunderstorms, hail (light)",
        97 => "Thunderstorms, heavy",
        99 => "Thunderstorms, hail (heavy)",
        other => return Err(AmbientWeatherRefusal::UnknownWeatherCode(other)),
    })
}

fn validate(
    ty: &CheckedNativeType,
    value: &StructuredInfoValue,
) -> Result<(), AmbientWeatherRefusal> {
    if value.value_type() != &ty.value_type {
        return Err(AmbientWeatherRefusal::NativeContract);
    }
    rust_binding::validate_native_contracts(value, &ty.value_contracts)
        .and_then(|()| rust_binding::validate_native_invariants(value, &ty.invariants))
        .map_err(|_| AmbientWeatherRefusal::NativeContract)
}

fn primitive<T: rust_binding::NativePrimitive>(
    ty: &StructuredInfoType,
    value: T,
) -> Result<StructuredInfoValue, AmbientWeatherRefusal> {
    rust_binding::primitive_into_structured(ty.clone(), &value)
        .map_err(|_| AmbientWeatherRefusal::NativeContract)
}

fn record(
    ty: &StructuredInfoType,
    mut make: impl FnMut(
        &str,
        &StructuredInfoType,
    ) -> Result<StructuredInfoValue, AmbientWeatherRefusal>,
) -> Result<StructuredInfoValue, AmbientWeatherRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(AmbientWeatherRefusal::NativeContract);
    };
    let values = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(field.name(), make(field.name(), field.value_type())?)
                .map_err(|_| AmbientWeatherRefusal::NativeContract)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::record(ty.clone(), values)
        .map_err(|_| AmbientWeatherRefusal::NativeContract)
}

fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, AmbientWeatherRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(AmbientWeatherRefusal::WrongReading);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or(AmbientWeatherRefusal::WrongReading)
}

#[cfg(test)]
#[path = "ambient_weather/tests.rs"]
mod tests;
