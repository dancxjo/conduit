//! Compose the checked clock/weather value from a correlated browser reply.
use super::CivilClockReply;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
    StructuredInfoTypeShape,
};

pub(crate) const OBSERVATION_BYTES: usize = 4096;
const RULE_PROVIDER: &str = "browser/intl-date-time-format";

pub(crate) struct PreparedObservation {
    record: Composer,
    fields: [Composer; 6],
    weather: Box<[u8]>,
    timezone: String,
}

impl PreparedObservation {
    pub(crate) fn prepare(timezone: String) -> Result<Self, String> {
        let document = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = &document
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientClockWeather")
            .ok_or("missing AmbientClockWeather type")?
            .value_type;
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err("ambient clock/weather type must be a record".into());
        };
        let [date, time, instant, zone, rules, offset, weather] = fields else {
            return Err("unexpected ambient clock/weather fields".into());
        };
        let fields = [
            (date, "local_date", 10),
            (time, "local_time", 8),
            (instant, "observed_at_unix_ms", 8),
            (zone, "timezone", 64),
            (rules, "timezone_rule_set", 80),
            (offset, "utc_offset_minutes", 8),
        ]
        .map(|(field, expected, maximum)| {
            if field.name() != expected {
                return Err(format!("unexpected clock field, expected {expected}"));
            }
            let ty = field.value_type();
            let capacity = ty.canonical_bytes().map_err(debug_error)?.len() + 5 + maximum;
            Composer::new(ty, capacity).map_err(debug_error)
        });
        let [date, time, instant, zone, rules, offset] = fields;
        if weather.name() != "weather" {
            return Err("missing ambient weather field".into());
        }
        let weather_type = weather.value_type();
        let StructuredInfoTypeShape::Variant { cases, .. } = weather_type.shape() else {
            return Err("ambient weather type must be a variant".into());
        };
        let unavailable = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("missing weather unavailable case")?;
        let reason = b"Weather provider unavailable";
        let text_type = unavailable.payload_type();
        let capacity = text_type.canonical_bytes().map_err(debug_error)?.len() + 5 + reason.len();
        let mut text = Composer::new(text_type, capacity).map_err(debug_error)?;
        let payload = validate(text.leaf(reason).map_err(debug_error)?).map_err(debug_error)?;
        let mut weather = Composer::new(weather_type, OBSERVATION_BYTES).map_err(debug_error)?;
        let weather = weather
            .variant("unavailable", payload)
            .map_err(debug_error)?
            .into();
        Ok(Self {
            record: Composer::new(ty, OBSERVATION_BYTES).map_err(debug_error)?,
            fields: [date?, time?, instant?, zone?, rules?, offset?],
            weather,
            timezone,
        })
    }

    pub(crate) fn prepared_heap_bytes(&self) -> usize {
        self.record.prepared_heap_bytes()
            + self
                .fields
                .iter()
                .map(Composer::prepared_heap_bytes)
                .sum::<usize>()
            + self.weather.len()
            + self.timezone.capacity()
    }

    pub(crate) fn encoded(&self) -> &[u8] {
        self.record.encoded()
    }

    pub(crate) fn compose(&mut self, reply: CivilClockReply<'_>) -> Result<&[u8], String> {
        let instant = reply.observed_at_unix_ms.to_le_bytes();
        let offset = i64::from(reply.utc_offset_minutes).to_le_bytes();
        let mut input = [
            reply.local_date.as_bytes(),
            reply.local_time.as_bytes(),
            instant.as_slice(),
            self.timezone.as_bytes(),
            RULE_PROVIDER.as_bytes(),
            offset.as_slice(),
        ]
        .into_iter();
        let [date, time, instant, zone, rules, offset] = self.fields.each_mut().map(|field| {
            let encoded = field.leaf(input.next().unwrap()).map_err(debug_error)?;
            validate(encoded).map_err(debug_error)
        });
        let weather = validate(&self.weather).map_err(debug_error)?;
        self.record
            .record(&[date?, time?, instant?, zone?, rules?, offset?, weather])
            .map_err(debug_error)
    }
}

fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_exact_clock_and_explicit_weather_absence() {
        let mut prepared = PreparedObservation::prepare("America/Los_Angeles".into()).unwrap();
        assert!(prepared.prepared_heap_bytes() >= OBSERVATION_BYTES);
        let encoded = prepared
            .compose(CivilClockReply {
                observed_at_unix_ms: 1_793_523_600_876,
                utc_offset_minutes: -480,
                local_date: "2026-11-01",
                local_time: "01:00:00",
            })
            .unwrap();
        let observation = validate(encoded).unwrap();
        assert_eq!(
            observation
                .record_field("local_time")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/text")
                .unwrap(),
            b"01:00:00"
        );
        assert_eq!(
            observation
                .record_field("weather")
                .unwrap()
                .unwrap()
                .variant_tag()
                .unwrap(),
            "unavailable"
        );
    }
}
