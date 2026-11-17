//! Prepared freshness projection of one checked, retained provider reading.
use super::*;
use conduit_core::{PreparedStructuredComposer, validate_canonical_structured_value as canonical};

const MAXIMUM_BYTES: usize = 4096;

/// No acquisition, scheduling or authority: the caller admits this owned
/// backing and supplies a newly prepared instance when its reading changes.
pub struct PreparedAmbientWeatherWire {
    reading: Box<[u8]>,
    observed: u64,
    expires: u64,
    fresh: Payload,
    stale: Payload,
    weather: PreparedStructuredComposer,
    unavailable: Box<[u8]>,
}
pub(super) struct Payload {
    record: PreparedStructuredComposer,
    reference: PreparedStructuredComposer,
    reading_first: bool,
}
impl Payload {
    pub(super) fn prepare(ty: &StructuredInfoType) -> Result<Self, AmbientWeatherRefusal> {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err(AmbientWeatherRefusal::NativeContract);
        };
        let reading_first = match fields {
            [a, b] if a.name() == "reading" && b.name() == "reference_at_unix_ms" => true,
            [a, b] if b.name() == "reading" && a.name() == "reference_at_unix_ms" => false,
            _ => return Err(AmbientWeatherRefusal::NativeContract),
        };
        let reference_type = fields
            .iter()
            .find(|f| f.name() == "reference_at_unix_ms")
            .ok_or(AmbientWeatherRefusal::NativeContract)?
            .value_type();
        Ok(Self {
            record: PreparedStructuredComposer::new(ty, MAXIMUM_BYTES)
                .map_err(|_| AmbientWeatherRefusal::NativeContract)?,
            reference: PreparedStructuredComposer::new(reference_type, MAXIMUM_BYTES)
                .map_err(|_| AmbientWeatherRefusal::NativeContract)?,
            reading_first,
        })
    }
    pub(super) fn encode(
        &mut self,
        reading: &[u8],
        now: u64,
    ) -> Result<&[u8], AmbientWeatherRefusal> {
        let reference = self
            .reference
            .leaf(&now.to_le_bytes())
            .map_err(|_| AmbientWeatherRefusal::NativeContract)?;
        let reference = canonical(reference).map_err(|_| AmbientWeatherRefusal::NativeContract)?;
        let reading = canonical(reading).map_err(|_| AmbientWeatherRefusal::NativeContract)?;
        let fields = if self.reading_first {
            [reading, reference]
        } else {
            [reference, reading]
        };
        self.record
            .record(&fields)
            .map_err(|_| AmbientWeatherRefusal::NativeContract)
    }
    pub(super) fn heap_bytes(&self) -> usize {
        self.record.prepared_heap_bytes() + self.reference.prepared_heap_bytes()
    }
}
impl PreparedAmbientWeatherWire {
    /// Validates the complete native reading, including refinements/invariants,
    /// before retaining it. Preparation allocates; `encode` never grows storage.
    pub fn prepare(reading: &StructuredInfoValue) -> Result<Self, AmbientWeatherRefusal> {
        let provider = PreparedOpenMeteoWeather::prepare("Seattle", 900_000)?;
        let observed =
            rust_binding::primitive_from_structured(field(reading, "observed_at_unix_ms")?)
                .map_err(|_| AmbientWeatherRefusal::WrongReading)?;
        provider.classify(reading, observed)?;
        let expires =
            rust_binding::primitive_from_structured(field(reading, "usable_until_unix_ms")?)
                .map_err(|_| AmbientWeatherRefusal::WrongReading)?;
        Ok(Self {
            reading: reading
                .canonical_bytes()
                .map_err(|_| AmbientWeatherRefusal::NativeContract)?
                .into_boxed_slice(),
            observed,
            expires,
            fresh: Payload::prepare(&provider.fresh.value_type)?,
            stale: Payload::prepare(&provider.stale.value_type)?,
            weather: PreparedStructuredComposer::new(&provider.weather.value_type, MAXIMUM_BYTES)
                .map_err(|_| AmbientWeatherRefusal::NativeContract)?,
            unavailable: provider
                .unavailable("Clock precedes weather validity")?
                .canonical_bytes()
                .map_err(|_| AmbientWeatherRefusal::NativeContract)?
                .into_boxed_slice(),
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.reading.len()
            + self.unavailable.len()
            + self.fresh.heap_bytes()
            + self.stale.heap_bytes()
            + self.weather.prepared_heap_bytes()
    }
    pub fn encode(&mut self, now: u64) -> Result<&[u8], AmbientWeatherRefusal> {
        if now < self.observed {
            return Ok(&self.unavailable);
        }
        let (tag, payload) = if now <= self.expires {
            ("fresh", &mut self.fresh)
        } else {
            ("stale", &mut self.stale)
        };
        let bytes = payload.encode(&self.reading, now)?;
        self.weather
            .variant(
                tag,
                canonical(bytes).map_err(|_| AmbientWeatherRefusal::NativeContract)?,
            )
            .map_err(|_| AmbientWeatherRefusal::NativeContract)
    }
}
