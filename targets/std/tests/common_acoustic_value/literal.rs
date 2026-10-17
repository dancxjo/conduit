//! Transparent fixture serialization of a Native seed into authored literals.
use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape};
pub fn literal(value: &StructuredInfoValue) -> String {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => {
            let StructuredInfoTypeShape::Leaf(kind) = value.value_type().shape() else {
                panic!("leaf")
            };
            match kind.as_str() {
                "value/empty" => "empty".into(),
                "value/bool" => {
                    if bytes == [0] {
                        "false".into()
                    } else {
                        "true".into()
                    }
                }
                "value/u32" => u32::from_le_bytes(bytes.try_into().unwrap()).to_string(),
                "value/i32" => i32::from_le_bytes(bytes.try_into().unwrap()).to_string(),
                "value/u64" => u64::from_le_bytes(bytes.try_into().unwrap()).to_string(),
                "value/i64" => i64::from_le_bytes(bytes.try_into().unwrap()).to_string(),
                "value/i128" => i128::from_le_bytes(bytes.try_into().unwrap()).to_string(),
                "value/text" => {
                    serde_json::to_string(core::str::from_utf8(bytes).unwrap()).unwrap()
                }
                kind => panic!("unhandled fixture primitive {kind}"),
            }
        }
        StructuredInfoValueShape::Record(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name(), literal(field.value())))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        StructuredInfoValueShape::Variant { tag, payload } => {
            format!("{tag}({})", literal(payload))
        }
        StructuredInfoValueShape::Collection(values) => format!(
            "[{}]",
            values.iter().map(literal).collect::<Vec<_>>().join(", ")
        ),
    }
}
