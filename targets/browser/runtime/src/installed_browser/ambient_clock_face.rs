//! Present the admitted clock observation through the ordinary browser Show.
use super::{
    factory::{
        manifestation_label, validate_placement, BrowserHostResult, BrowserInstallation,
        BrowserManifestation,
    },
    BrowserBack,
};
use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    PlannedGear, FACE_RESOURCE_CLASS,
};
use conduit_semantic_catalog as catalog;

const ID: &str = "browser/face-ambient-clock-weather@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: ID,
    offer,
    prepare,
    perform: Some(present),
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        catalog::ambient_clock_face_kind().expect("checked ambient Face Kind"),
        Back {
            capability_id: CapabilityId::from(ID),
            execution_profile_id: ExecutionProfileId::from(ID),
            implementation_id: ImplementationId::from(ID),
            artifact_id: ArtifactId::from("conduit-browser-runtime/ambient-clock-face@1"),
            host_calls: vec![present_host_call_requirement(
                kind_id(catalog::AMBIENT_CLOCK_FACE_KIND),
                catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
            )],
            resource_requirements: vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::face(
        catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
        1,
    ))
}

fn present(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: manifestation_label(placement)?,
            kind_id: catalog::AMBIENT_CLOCK_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

pub(crate) fn text(input: &[u8]) -> Result<String, String> {
    let value = conduit_core::validate_canonical_structured_value(input)
        .map_err(|error| format!("decode ambient Face: {error:?}"))?;
    let date = label(field(value, "local_date")?)?;
    let time = label(field(value, "local_time")?)?;
    let timezone = label(field(value, "timezone")?)?;
    let weather = weather_text(field(value, "weather")?)?;
    Ok(format!("{date} · {time} · {timezone} · {weather}"))
}

type Observation<'a> = conduit_core::ValidatedCanonicalStructuredValue<'a>;

fn field<'a>(value: Observation<'a>, name: &str) -> Result<Observation<'a>, String> {
    value
        .record_field(name)
        .map_err(debug_error)?
        .ok_or_else(|| format!("missing ambient Face field: {name}"))
}

fn label(value: Observation<'_>) -> Result<&str, String> {
    let bytes = value
        .primitive_bytes(conduit_core::TEXT_INFO_ID)
        .map_err(debug_error)?;
    core::str::from_utf8(bytes).map_err(debug_error)
}

fn weather_text(weather: Observation<'_>) -> Result<String, String> {
    let status = weather.variant_tag().map_err(debug_error)?;
    let payload = weather
        .variant_payload(status)
        .map_err(debug_error)?
        .ok_or("missing weather variant payload")?;
    match status {
        "unavailable" => Ok(format!("Weather unavailable · {}", label(payload)?)),
        "fresh" | "stale" => {
            let reading = field(payload, "reading")?;
            let location = label(field(reading, "location")?)?;
            let conditions = label(field(reading, "conditions")?)?;
            let provider = label(field(reading, "attribution")?)?;
            let temperature = i64::from_le_bytes(
                field(reading, "temperature_celsius_milli")?
                    .primitive_bytes("value/i64")
                    .map_err(debug_error)?
                    .try_into()
                    .map_err(debug_error)?,
            );
            let temperature = temperature as f64 / 1000.0;
            Ok(format!(
                "Weather {status} · {location} · {conditions} · {temperature} °C · {provider}"
            ))
        }
        _ => Err("unknown ambient weather status".into()),
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    };
    use conduit_plot::rust_binding::{primitive_into_structured, NativePrimitive};

    #[test]
    fn available_weather_preserves_reading_and_distinguishes_staleness() {
        let types = catalog::prepare_ambient_display_types().unwrap();
        let weather = types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientWeather")
            .unwrap();
        let StructuredInfoTypeShape::Variant { cases, .. } = weather.value_type.shape() else {
            panic!("expected semantic weather cases");
        };
        for (status, reference) in [("fresh", 2_u64), ("stale", 3)] {
            let case = cases.iter().find(|case| case.tag() == status).unwrap();
            let payload = record(case.payload_type(), |name, ty| match name {
                "reference_at_unix_ms" => primitive(ty, &reference),
                "reading" => record(ty, |name, ty| match name {
                    "location" => primitive(ty, &String::from("Seattle")),
                    "conditions" => primitive(ty, &String::from("Clear")),
                    "attribution" => primitive(ty, &String::from("Example weather service")),
                    "temperature_celsius_milli" => primitive(ty, &-1250_i64),
                    "observed_at_unix_ms" => primitive(ty, &1_u64),
                    "usable_until_unix_ms" => primitive(ty, &2_u64),
                    _ => panic!("unexpected reading field"),
                }),
                _ => panic!("unexpected weather field"),
            });
            let encoded = StructuredInfoValue::variant(weather.value_type.clone(), status, payload)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let value = conduit_core::validate_canonical_structured_value(&encoded).unwrap();
            assert_eq!(
                weather_text(value).unwrap(),
                format!("Weather {status} · Seattle · Clear · -1.25 °C · Example weather service")
            );
        }
    }

    fn primitive<T: NativePrimitive>(ty: &StructuredInfoType, value: &T) -> StructuredInfoValue {
        primitive_into_structured(ty.clone(), value).unwrap()
    }

    fn record(
        ty: &StructuredInfoType,
        mut value: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
    ) -> StructuredInfoValue {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            panic!("expected semantic weather record");
        };
        let fields = fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(field.name(), value(field.name(), field.value_type()))
                    .unwrap()
            })
            .collect();
        StructuredInfoValue::record(ty.clone(), fields).unwrap()
    }
}
