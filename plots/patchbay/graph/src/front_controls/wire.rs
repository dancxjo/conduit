//! Owned decoding preserves the public serializer's externally tagged enum.
//! Only canonical projector literals become static references; input is never leaked.
use super::FaceControlKind;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Deserializer};

// Only these projector literals are static. Other string-valued controls retain
// owned decoding, and escaped equivalent JSON spellings still reach visit_str.
struct Literal<const SCALAR: bool>(&'static str);
impl<'de, const SCALAR: bool> Deserialize<'de> for Literal<SCALAR> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<const SCALAR: bool>;
        impl<'de, const SCALAR: bool> serde::de::Visitor<'de> for Visitor<SCALAR> {
            type Value = Literal<SCALAR>;
            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a string")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match (SCALAR, value) {
                    (false, "false") => Ok(Literal("false")),
                    (false, "true") => Ok(Literal("true")),
                    (true, "µ") => Ok(Literal("µ")),
                    _ => Err(E::custom(if SCALAR {
                        "noncanonical scalar unit"
                    } else {
                        "noncanonical Boolean choices"
                    })),
                }
            }
            fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                let value = core::str::from_utf8(value)
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Bytes(value), &self))?;
                self.visit_str(value)
            }
        }
        deserializer.deserialize_str(Visitor::<SCALAR>)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum OwnedFaceControlKind {
    BooleanChoice {
        choices: [Literal<false>; 2],
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
        unit: Literal<true>,
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
                let choices = choices.map(|literal| literal.0);
                if choices != ["false", "true"] {
                    return Err(serde::de::Error::custom("noncanonical Boolean choices"));
                }
                Self::BooleanChoice { choices }
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
            } => Self::ScalarNumber {
                minimum,
                maximum,
                unit: unit.0,
            },
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
