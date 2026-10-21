//! Owned decoding preserves the public serializer's externally tagged enum.
//! Only canonical projector literals become static references; input is never leaked.
use super::FaceControlKind;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum OwnedFaceControlKind {
    BooleanChoice {
        choices: [String; 2],
    },
    TextChoice {
        choices: Vec<String>,
    },
    Number {
        minimum: u64,
        maximum: u64,
        unit: Option<String>,
    },
    ScalarNumber {
        minimum: i64,
        maximum: i64,
        unit: String,
    },
    Range {
        minimum: u64,
        maximum: u64,
        unit: Option<String>,
    },
    StructuredSource,
    PhysicalSource {
        maximum_bytes: u32,
    },
    ShortText {
        maximum_bytes: u32,
    },
}

impl<'de> Deserialize<'de> for FaceControlKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match OwnedFaceControlKind::deserialize(deserializer)? {
            OwnedFaceControlKind::BooleanChoice { choices } => {
                if choices != ["false", "true"] {
                    return Err(serde::de::Error::custom("noncanonical Boolean choices"));
                }
                Self::BooleanChoice {
                    choices: ["false", "true"],
                }
            }
            OwnedFaceControlKind::TextChoice { choices } => Self::TextChoice { choices },
            OwnedFaceControlKind::Number {
                minimum,
                maximum,
                unit,
            } => Self::Number {
                minimum,
                maximum,
                unit,
            },
            OwnedFaceControlKind::ScalarNumber {
                minimum,
                maximum,
                unit,
            } => {
                if unit != "µ" {
                    return Err(serde::de::Error::custom("noncanonical scalar unit"));
                }
                Self::ScalarNumber {
                    minimum,
                    maximum,
                    unit: "µ",
                }
            }
            OwnedFaceControlKind::Range {
                minimum,
                maximum,
                unit,
            } => Self::Range {
                minimum,
                maximum,
                unit,
            },
            OwnedFaceControlKind::StructuredSource => Self::StructuredSource,
            OwnedFaceControlKind::PhysicalSource { maximum_bytes } => {
                Self::PhysicalSource { maximum_bytes }
            }
            OwnedFaceControlKind::ShortText { maximum_bytes } => Self::ShortText { maximum_bytes },
        })
    }
}
