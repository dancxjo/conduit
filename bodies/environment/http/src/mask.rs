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
    face_response(face, ap_ready, now, json).expect("prepared semantic Face revision")
}

/// Interpret this exact immutable Face. Root separately supplies current reader
/// custody, clock and route admission; the Mask owns units and text only.
/// Other Body subjects cannot supply or shadow environmental properties.
pub fn face_response(
    face: &conduit_face::Face,
    ap_ready: bool,
    now: u64,
    json: bool,
) -> Result<heapless::String<2048>, conduit_seven_segment_mask::FaceProjectionRefusal> {
    Ok(PreparedHttpShow::prepare(face)?.response(ap_ready, now, json))
}

/// Resolved HTTP Show material for one exact Face revision. Quantities stay
/// upstream; only their bounded formatted coordinates are retained here.
/// This is inert Mask material, not a Face owner, clock, route or read permit.
/// Its caller must withdraw it when custody, basis or freshness lifetime ends.
pub struct PreparedHttpShow {
    identity: [u8; 64],
    revision: u64,
    sequence: u64,
    time: HttpShowTime,
    fresh: bool,
    current: bool,
    status: &'static str,
    temperature: Option<heapless::String<48>>,
    humidity: Option<heapless::String<48>>,
}
/// Coordinate labels are preserved from the Face; observed time is never
/// presented as acquisition time. Root separately supplies the matching clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpShowTime {
    SampledAtBootMs(u64),
    ObservedAtBootMs(u64),
    Unavailable,
}
struct JsonTime(Option<u64>);
impl core::fmt::Display for JsonTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("null"),
        }
    }
}
const LABELS: &[&str] = &[
    "warming",
    "available",
    "unavailable",
    "capture-timeout",
    "provider-failed",
    "geometry-refused",
    "short-frame",
    "malformed-length",
    "checksum",
    "invalid-payload",
];
impl PreparedHttpShow {
    pub fn face_identity(&self) -> &str {
        // Copied exclusively from a validated Face's immutable UTF-8 identity.
        core::str::from_utf8(&self.identity).expect("prepared Face identity")
    }
    pub fn face_revision(&self) -> u64 {
        self.revision
    }
    pub fn time_coordinate(&self) -> HttpShowTime {
        self.time
    }
    pub fn prepare(
        face: &conduit_face::Face,
    ) -> Result<Self, conduit_seven_segment_mask::FaceProjectionRefusal> {
        conduit_seven_segment_mask::PreparedEnvironmentFace::prepare(face, "environment")?;
        let value = |name| {
            face.properties
                .iter()
                .find(|p| p.subject == "environment" && p.name == name)
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
        let time = match value("sampled-at-boot-ms") {
            Some(conduit_face::FacePropertyValue::Count(value)) => {
                HttpShowTime::SampledAtBootMs(*value)
            }
            _ => match value("observed-at-boot-ms") {
                Some(conduit_face::FacePropertyValue::Count(value)) => {
                    HttpShowTime::ObservedAtBootMs(*value)
                }
                _ => HttpShowTime::Unavailable,
            },
        };
        let sequence = count("sequence");
        let status = text("sensor-failure")
            .or_else(|| text("sensor-status"))
            .and_then(|value| LABELS.iter().copied().find(|label| *label == value))
            .ok_or(conduit_seven_segment_mask::FaceProjectionRefusal::WrongPropertyType)?;
        let fresh = matches!(
            value("fresh"),
            Some(conduit_face::FacePropertyValue::Flag(true))
        );
        let current = fresh && text("sensor-status") == Some("available");
        let temperature = match quantity("temperature") {
            Some(value) => Some(
                coordinate(value, Unit::Fahrenheit)
                    .ok_or(conduit_seven_segment_mask::FaceProjectionRefusal::InvalidQuantity)?,
            ),
            None if fresh || text("sensor-status") == Some("available") => {
                return Err(conduit_seven_segment_mask::FaceProjectionRefusal::MissingProperty);
            }
            None => None,
        };
        let humidity = quantity("relative-humidity").and_then(|q| coordinate(q, Unit::Percent));
        Ok(Self {
            identity: face
                .identity
                .as_str()
                .as_bytes()
                .try_into()
                .map_err(|_| conduit_seven_segment_mask::FaceProjectionRefusal::InvalidFace)?,
            revision: face.revision,
            sequence,
            time,
            fresh,
            current,
            status,
            temperature,
            humidity,
        })
    }
    /// Render owned finite material. `now` changes age text only; it cannot
    /// refresh the Face revision or extend its admitted manifestation lifetime.
    pub fn response(&self, ap_ready: bool, now: u64, json: bool) -> heapless::String<2048> {
        let Self {
            sequence,
            time,
            fresh,
            current,
            status,
            temperature,
            humidity,
            ..
        } = self;
        let (sampled_ms, observed_ms, reference, basis) = match *time {
            HttpShowTime::SampledAtBootMs(value) => (Some(value), None, Some(value), "sampled"),
            HttpShowTime::ObservedAtBootMs(value) => (None, Some(value), Some(value), "observed"),
            HttpShowTime::Unavailable => (None, None, None, "unavailable"),
        };
        let age = reference.map(|value| now.saturating_sub(value));
        let current = *current;
        let mut body = heapless::String::<1536>::new();
        if json {
            let _ = write!(body, "{{\"face_identity\":\"{}\",\"face_revision\":{},\"sequence\":{},\"status\":\"{}\",\"fresh\":{},\"current\":{},\"temperature_f\":{},\"relative_humidity_percent\":{},\"sampled_at_boot_ms\":{},\"observed_at_boot_ms\":{},\"age_basis\":\"{}\",\"age_ms\":{},\"ap_ready\":{}}}", self.face_identity(), self.revision, sequence, status, fresh, current,
            temperature.as_ref().map_or("null",|v|v.as_str()), humidity.as_ref().map_or("null",|v|v.as_str()), JsonTime(sampled_ms), JsonTime(observed_ms), basis, JsonTime(age), ap_ready);
        } else {
            let _ = write!(body, "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta http-equiv=refresh content=5><title>Conduit Climate</title><style>body{{font:20px system-ui;max-width:34rem;margin:3rem auto;padding:1rem;background:#f5f4ee;color:#203c38}}strong{{font-size:3rem}}small{{display:block;margin-top:1rem}}</style><h1>Conduit Climate</h1><p>{}</p><p><strong>{} &deg;F</strong></p><p>{}% relative humidity</p><small>Sensor: {}. Reading age: {} seconds. Sample {}.</small><small>Refreshes every 5 seconds.</small><small><a href=/reading>Reading as JSON</a></small></html>",
            if current { "Current reading" } else if temperature.is_some() { "Last known reading — not current" } else { "Waiting for a valid sensor reading" },
            temperature.as_ref().map_or("—",|v|v.as_str()), humidity.as_ref().map_or("—",|v|v.as_str()), status, age.unwrap_or(0)/1000, sequence);
        }
        let mut out = heapless::String::<2048>::new();
        write!(out, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", if json { "application/json" } else { "text/html; charset=utf-8" }, body.len(), body).unwrap();
        out
    }
}
