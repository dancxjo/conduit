//! Lossless, bounded configuration transport for firmware evidence only.
//! Decoding validates canonical configuration bytes, not renderer authority.
use alloc::{string::String, vec::Vec};
use conduit_core::{MAXIMUM_STRUCTURED_CANONICAL_BYTES, StructuredConfigurationValue};
use conduit_face::ConfiguredMaskConfiguration;
use serde_json::{Value, json};

pub const CONFIGURATION_ENCODING: &str = "configured-mask-canonical-hex@1";
const INVALID: &str = "firmware-face-configuration-invalid";

pub fn encode_configuration(
    configuration: &ConfiguredMaskConfiguration,
) -> Result<Value, &'static str> {
    let mut value = serde_json::to_value(configuration).map_err(|_| INVALID)?;
    for name in ["recipe", "look", "layout"] {
        let Some(field) = value.get_mut(name) else {
            continue;
        };
        let bytes = validated_bytes(field, false)?;
        let mut hex = String::with_capacity(bytes.len().checked_mul(2).ok_or(INVALID)?);
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in &bytes {
            hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
            hex.push(char::from(DIGITS[usize::from(byte & 15)]));
        }
        field["canonical_value"] =
            json!({"encoding":"hex@1", "byte_length":bytes.len(), "value":hex});
    }
    value
        .as_object_mut()
        .ok_or(INVALID)?
        .insert("evidence_encoding".into(), CONFIGURATION_ENCODING.into());
    Ok(value)
}

/// Reconstruct the original JSON configuration, accepting legacy byte arrays.
/// Null denotes the absence of a configured renderer in historical records.
pub fn decode_configuration(value: &Value) -> Result<Value, &'static str> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let object = value.as_object().ok_or(INVALID)?;
    let marked = match object.get("evidence_encoding") {
        None => false,
        Some(Value::String(marker)) if marker == CONFIGURATION_ENCODING => true,
        _ => return Err(INVALID),
    };
    validate_metadata(value)?;
    let mut fields = Vec::with_capacity(3);
    for name in ["recipe", "look", "layout"] {
        let Some(field) = value.get(name) else {
            if name == "layout" {
                continue;
            }
            return Err(INVALID);
        };
        let bytes = validated_bytes(field, marked)?;
        fields.push((name, bytes));
    }
    let mut decoded = value.clone();
    for (name, bytes) in fields {
        decoded[name]["canonical_value"] = json!(bytes);
    }
    decoded
        .as_object_mut()
        .ok_or(INVALID)?
        .remove("evidence_encoding");
    Ok(decoded)
}

fn validate_metadata(value: &Value) -> Result<(), &'static str> {
    for name in [
        "placement_id",
        "host_id",
        "boot_id",
        "execution_profile_id",
        "implementation_id",
    ] {
        if value
            .get(name)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(INVALID);
        }
    }
    let component = value
        .get("component")
        .and_then(Value::as_object)
        .ok_or(INVALID)?;
    for name in ["source_document_id", "checked_plot_id", "fore_id"] {
        if component
            .get(name)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(INVALID);
        }
    }
    let path = value
        .get("invocation_path")
        .and_then(Value::as_array)
        .ok_or(INVALID)?;
    if path
        .iter()
        .any(|part| part.as_str().is_none_or(str::is_empty))
    {
        return Err(INVALID);
    }
    Ok(())
}

fn validated_bytes(value: &Value, marked: bool) -> Result<Vec<u8>, &'static str> {
    let profile = value
        .get("profile")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(INVALID)?;
    let payload = value.get("canonical_value").ok_or(INVALID)?;
    let bytes = if marked {
        let object = payload.as_object().ok_or(INVALID)?;
        if object.len() != 3 || object.get("encoding").and_then(Value::as_str) != Some("hex@1") {
            return Err(INVALID);
        }
        let length = object
            .get("byte_length")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(INVALID)?;
        if length == 0 || length > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(INVALID);
        }
        let text = object.get("value").and_then(Value::as_str).ok_or(INVALID)?;
        if text.len() != length.checked_mul(2).ok_or(INVALID)? {
            return Err(INVALID);
        }
        let mut bytes = Vec::with_capacity(length);
        for pair in text.as_bytes().chunks_exact(2) {
            bytes.push((digit(pair[0])? << 4) | digit(pair[1])?);
        }
        bytes
    } else {
        let array = payload.as_array().ok_or(INVALID)?;
        if array.is_empty() || array.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(INVALID);
        }
        array
            .iter()
            .map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()).ok_or(INVALID))
            .collect::<Result<Vec<_>, _>>()?
    };
    StructuredConfigurationValue::new(profile.into(), bytes.clone()).ok_or(INVALID)?;
    Ok(bytes)
}
fn digit(byte: u8) -> Result<u8, &'static str> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(INVALID),
    }
}

#[cfg(test)]
mod tests;
