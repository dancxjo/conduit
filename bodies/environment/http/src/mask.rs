//! Finite HTTP Show from one immutable semantic Face revision.
//! Units and textual representation belong here, downstream of observation.
use conduit_core::{Quantity, Unit};
use core::fmt::Write;

fn coordinate(value: Quantity, unit: Unit) -> Option<heapless::String<48>> {
    let (coefficient, exponent) = value.convert_to_unit(unit).ok()?;
    // DHT11 quantity Forms admit at most hundredths and bounded physical ranges.
    // Refuse unexpected numeric extents instead of truncating them.
    if !(-6..=6).contains(&exponent) {
        return None;
    }
    let mut text = heapless::String::<48>::new();
    if exponent >= 0 {
        write!(
            text,
            "{}",
            coefficient.checked_mul(10_i128.checked_pow(exponent as u32)?)?
        )
        .ok()?;
    } else {
        let scale = 10_u128.pow((-exponent) as u32);
        let magnitude = coefficient.unsigned_abs();
        if coefficient < 0 {
            text.push('-').ok()?;
        }
        write!(
            text,
            "{}.{:0width$}",
            magnitude / scale,
            magnitude % scale,
            width = (-exponent) as usize
        )
        .ok()?;
    }
    Some(text)
}

pub(crate) fn response(
    face: &conduit_face::Face,
    ap_ready: bool,
    now: u64,
    json: bool,
) -> heapless::String<2048> {
    face.validate().expect("prepared semantic Face revision");
    let value = |name| {
        face.properties
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    };
    let count = |name| match value(name) {
        Some(conduit_face::FacePropertyValue::Count(v)) => *v,
        _ => 0,
    };
    let text = |name| match value(name) {
        Some(conduit_face::FacePropertyValue::Text(v)) => Some(v.as_str()),
        _ => None,
    };
    let quantity = |name| match value(name) {
        Some(conduit_face::FacePropertyValue::TypedValue { contract, bytes })
            if contract.validate(bytes).is_ok() =>
        {
            Quantity::decode(bytes).ok()
        }
        _ => None,
    };
    let sampled_ms = count("sampled-at-boot-ms");
    let sequence = count("sequence");
    let status = text("sensor-failure")
        .or_else(|| text("sensor-status"))
        .unwrap_or("invalid-face");
    let age = now.saturating_sub(sampled_ms);
    let fresh = matches!(
        value("fresh"),
        Some(conduit_face::FacePropertyValue::Flag(true))
    );
    let current = fresh && text("sensor-status") == Some("available");
    let temperature = quantity("temperature").and_then(|q| coordinate(q, Unit::Fahrenheit));
    let humidity = quantity("relative-humidity").and_then(|q| coordinate(q, Unit::Percent));
    let mut body = heapless::String::<1536>::new();
    if json {
        let _ = write!(body, "{{\"face_identity\":\"{}\",\"face_revision\":{},\"sequence\":{},\"status\":\"{}\",\"fresh\":{},\"current\":{},\"temperature_f\":{},\"relative_humidity_percent\":{},\"sampled_at_boot_ms\":{},\"age_ms\":{},\"ap_ready\":{}}}", face.identity.as_str(), face.revision, sequence, status, fresh, current,
            temperature.as_ref().map_or("null",|v|v.as_str()), humidity.as_ref().map_or("null",|v|v.as_str()), sampled_ms, age, ap_ready);
    } else {
        let _ = write!(body, "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta http-equiv=refresh content=5><title>Conduit Climate</title><style>body{{font:20px system-ui;max-width:34rem;margin:3rem auto;padding:1rem;background:#f5f4ee;color:#203c38}}strong{{font-size:3rem}}small{{display:block;margin-top:1rem}}</style><h1>Conduit Climate</h1><p>{}</p><p><strong>{} &deg;F</strong></p><p>{}% relative humidity</p><small>Sensor: {}. Reading age: {} seconds. Sample {}.</small><small>Refreshes every 5 seconds; samples every 15 seconds. Local access point; no internet connection.</small><small><a href=/reading>Reading as JSON</a></small></html>",
            if current { "Current reading" } else if temperature.is_some() { "Last known reading — not current" } else { "Waiting for a valid sensor reading" },
            temperature.as_ref().map_or("—",|v|v.as_str()), humidity.as_ref().map_or("—",|v|v.as_str()), status, age/1000, sequence);
    }
    let mut out = heapless::String::<2048>::new();
    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", if json { "application/json" } else { "text/html; charset=utf-8" }, body.len(), body).unwrap();
    out
}
