//! Private mechanical native-data authoring adapter, with no language policy.
use alloc::{
    format,
    string::{String, ToString},
};
use conduit_core::{
    primitive_info_kind, FixedInteger, PrimitiveInfoKind, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_DEPTH,
};
use core::fmt::Write;
pub(super) fn literal(value: &StructuredInfoValue) -> Result<String, String> {
    let mut result = String::new();
    write_value(value, &mut result, 0)?;
    Ok(result)
}
fn write_value(value: &StructuredInfoValue, out: &mut String, depth: usize) -> Result<(), String> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
        return Err("literal depth".into());
    }
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => {
            let mut ty = value.value_type();
            while let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
                ty = representation;
            }
            let StructuredInfoTypeShape::Leaf(kind) = ty.shape() else {
                return Err("literal leaf representation".into());
            };
            let kind = primitive_info_kind(kind.as_str()).ok_or("unsupported literal leaf")?;
            let text = match kind {
                PrimitiveInfoKind::Text => conduit_plot::text_startup_literal(
                    core::str::from_utf8(bytes).map_err(|_| "literal UTF8")?,
                ),
                PrimitiveInfoKind::Unit if bytes.is_empty() => "\"\"".into(),
                PrimitiveInfoKind::Bool => match bytes {
                    [0] => "false".into(),
                    [1] => "true".into(),
                    _ => return Err("literal Bool".into()),
                },
                PrimitiveInfoKind::Count => conduit_core::decode_count(bytes)
                    .map_err(|_| "literal Count")?
                    .to_string(),
                PrimitiveInfoKind::F32 => {
                    f32::from_le_bytes(bytes.try_into().map_err(|_| "literal F32")?).to_string()
                }
                PrimitiveInfoKind::F64 => {
                    f64::from_le_bytes(bytes.try_into().map_err(|_| "literal F64")?).to_string()
                }
                _ => {
                    let integer = FixedInteger::decode(kind, bytes)
                        .map_err(|_| "unsupported literal integer")?;
                    integer
                        .signed()
                        .map_or_else(
                            |_| integer.unsigned().map(|value| value.to_string()),
                            |value| Ok(value.to_string()),
                        )
                        .map_err(|_| "literal integer")?
                }
            };
            out.push_str(&text);
        }
        StructuredInfoValueShape::Collection(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(value, out, depth + 1)?;
            }
            out.push(']');
        }
        StructuredInfoValueShape::Record(fields) => {
            out.push('{');
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write!(out, "{}:", field.name()).map_err(|_| "literal formatting")?;
                write_value(field.value(), out, depth + 1)?;
            }
            out.push('}');
        }
        StructuredInfoValueShape::Variant { tag, payload } => {
            write!(out, "{tag}(").map_err(|_| "literal formatting")?;
            write_value(payload, out, depth + 1)?;
            out.push(')');
        }
    }
    if out.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(format!(
            "literal exceeds {MAXIMUM_STRUCTURED_CANONICAL_BYTES} bytes"
        ));
    }
    Ok(())
}
