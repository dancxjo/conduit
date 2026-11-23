//! A viewer-local expression of a typed weather observation. The complete
//! owner wording and typed facts remain available to the Details reader.
use alloc::{format, string::String};
use conduit_core::{Quantity, TEMPERATURE_INFO_ID, Unit};
use conduit_face::{Face, FacePropertyValue, FaceRole, FaceSubject};

fn property<'a>(
    face: &'a Face,
    subject: &FaceSubject,
    name: &str,
) -> Option<&'a FacePropertyValue> {
    face.properties
        .iter()
        .find(|p| p.subject == subject.identity && p.name == name)
        .map(|p| &p.value)
}

pub(super) fn text_fact<'a>(face: &'a Face, subject: &FaceSubject, name: &str) -> Option<&'a str> {
    match property(face, subject, name)? {
        FacePropertyValue::Text(value) => Some(value),
        _ => None,
    }
}

fn selected_temperature(
    face: &Face,
    subject: &FaceSubject,
    unit: Unit,
    precision: u8,
) -> Option<(i128, &'static str)> {
    let symbol = match unit {
        Unit::Celsius => "°C",
        Unit::Fahrenheit => "°F",
        _ => return None,
    };
    let FacePropertyValue::TypedValue { contract, bytes } = property(face, subject, "temperature")?
    else {
        return None;
    };
    if contract.value_kind.as_str() != TEMPERATURE_INFO_ID {
        return None;
    }
    // Document preparation has already validated the Face and its inline contracts.
    let quantity = Quantity::decode(bytes).ok()?;
    let value = conduit_face::rounded_quantity_coordinate(quantity, unit, precision).ok()?;
    Some((value, symbol))
}

pub(super) fn summary(face: &Face, subject: &FaceSubject, unit: Option<Unit>) -> Option<String> {
    if !matches!(&subject.role, FaceRole::Semantic(kind) if kind.as_str() == "weather/conditions") {
        return None;
    }
    let state = text_fact(face, subject, "availability")?;
    let location = text_fact(face, subject, "location")?;
    let conditions = text_fact(face, subject, "conditions")?;
    let (value, symbol) = selected_temperature(face, subject, unit?, 1)?;
    let magnitude = value.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    let whole = magnitude / 10;
    let tenth = magnitude % 10;
    match state {
        "fresh" => Some(format!(
            "{location}: {sign}{whole}.{tenth} {symbol}, {conditions}."
        )),
        "stale" => Some(format!(
            "Last reported for {location}: {sign}{whole}.{tenth} {symbol}, {conditions}. Weather is out of date."
        )),
        _ => None,
    }
}

pub(super) fn overlay(face: &Face, subject: &FaceSubject, unit: Option<Unit>) -> Option<String> {
    match text_fact(face, subject, "availability")? {
        "stale" => Some("Weather out of date".into()),
        "fresh" => {
            let condition = text_fact(face, subject, "conditions")
                .filter(|text| !text.trim().is_empty() && !text.chars().any(char::is_control));
            let temperature = unit
                .and_then(|unit| selected_temperature(face, subject, unit, 0))
                .map(|(value, symbol)| format!("{value}{symbol}"));
            let limit = conduit_face::MAX_GRAPHICS_TEXT_BYTES;
            match (temperature, condition) {
                (Some(value), Some(condition))
                    if value.len() + " · ".len() + condition.len() <= limit =>
                {
                    Some(format!("{value} · {condition}"))
                }
                (Some(value), _) => Some(value),
                (None, Some(condition)) if condition.len() <= limit => Some(condition.into()),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "weather/tests.rs"]
mod tests;
