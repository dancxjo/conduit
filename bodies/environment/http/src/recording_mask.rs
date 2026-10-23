//! Finite HTTP Show of the ordinary recorder Face. No recovery, storage, clock
//! or acknowledgement state is maintained by this Mask.
use conduit_face::{Face, FacePropertyValue as V, FaceRole};
use core::fmt::Write;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingMaskRefusal {
    InvalidFace,
    UnsupportedFace,
    Capacity,
}

/// Root supplies its actual committed Face. This prepares a Show only; it does
/// not install a route or make the recorder Host, Body or store available.
pub fn recording_response(
    face: &Face,
    json: bool,
) -> Result<heapless::String<2048>, RecordingMaskRefusal> {
    use RecordingMaskRefusal as E;
    face.validate().map_err(|_| E::InvalidFace)?;
    if !face
        .subjects
        .iter()
        .any(|s| s.identity == "environment-recording" && s.role == FaceRole::Info)
    {
        return Err(E::UnsupportedFace);
    }
    let value = |name| {
        face.properties
            .iter()
            .find(|p| p.subject == "environment-recording" && p.name == name)
            .map(|p| &p.value)
    };
    let count = |name| match value(name) {
        Some(V::Count(n)) => Ok(*n),
        _ => Err(E::UnsupportedFace),
    };
    let text = |name| match value(name) {
        Some(V::Text(s)) => Ok(s.as_str()),
        _ => Err(E::UnsupportedFace),
    };
    let status = text("recovery-status")?;
    let failure = text("recovery-failure")?;
    if !matches!(
        (status, failure),
        ("empty" | "recovered", "none") | ("refused", "invalid-content" | "capacity" | "cancelled")
    ) {
        return Err(E::UnsupportedFace);
    }
    let has_range = match value("has-range") {
        Some(V::Flag(v)) => *v,
        _ => return Err(E::UnsupportedFace),
    };
    let chunks = count("chunks")?;
    let samples = count("samples")?;
    let first = count("first-sequence")?;
    let last = count("last-sequence")?;
    let missing = count("missing-sequences")?;
    if has_range {
        if chunks == 0
            || chunks > u64::from(u16::MAX)
            || samples > u64::from(u32::MAX)
            || samples < chunks
            || first == 0
            || last < first
            || last - first < samples - 1
            || missing != last - first + 1 - samples
            || status == "empty"
        {
            return Err(E::UnsupportedFace);
        }
    } else if [chunks, samples, first, last, missing] != [0; 5] || status == "recovered" {
        return Err(E::UnsupportedFace);
    }
    let mut body = heapless::String::<1536>::new();
    if json {
        write!(body, "{{\"schema\":\"conduit.environment/recording-show@1\",\"face_identity\":\"{}\",\"face_revision\":{},\"recovery_status\":\"{status}\",\"recovery_failure\":\"{failure}\",\"has_range\":{has_range},\"chunks\":{chunks},\"samples\":{samples},\"first_sequence\":{first},\"last_sequence\":{last},\"missing_sequences\":{missing}}}", face.identity.as_str(), face.revision).map_err(|_| E::Capacity)?;
    } else {
        write!(body, "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Recording recovery</title><h1>Recording recovery</h1><p>Status: {status}. Reason: {failure}.</p><p>{samples} recovered samples in {chunks} chunks.</p>").map_err(|_| E::Capacity)?;
        if has_range {
            write!(
                body,
                "<p>Sequences {first} through {last}; {missing} missing within this range.</p>"
            )
            .map_err(|_| E::Capacity)?;
        } else {
            body.push_str("<p>No recovered range.</p>")
                .map_err(|_| E::Capacity)?;
        }
        body.push_str("</html>").map_err(|_| E::Capacity)?;
    }
    let mut response = heapless::String::<2048>::new();
    write!(response, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", if json { "application/json" } else { "text/html; charset=utf-8" }, body.len(), body).map_err(|_| E::Capacity)?;
    Ok(response)
}
