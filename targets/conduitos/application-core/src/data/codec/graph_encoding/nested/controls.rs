use super::*;
use ConfigurationValue as V;
use FaceControlKind as K;
pub(super) fn shape(v: Nested<'_>) -> Result<Shape<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Control(_) => Shape::Object(&["key", "value", "kind", "interaction"]),
        ControlKind(k) => match k {
            K::StructuredSource => Shape::String("StructuredSource"),
            K::BooleanChoice { .. } => Shape::Object(&["BooleanChoice"]),
            K::TextChoice { .. } => Shape::Object(&["TextChoice"]),
            K::Number { .. } => Shape::Object(&["Number"]),
            K::ScalarNumber { .. } => Shape::Object(&["ScalarNumber"]),
            K::Range { .. } => Shape::Object(&["Range"]),
            K::PhysicalSource { .. } => Shape::Object(&["PhysicalSource"]),
            K::ShortText { .. } => Shape::Object(&["ShortText"]),
        },
        KindPayload(k) => Shape::Object(match k {
            K::BooleanChoice { .. } | K::TextChoice { .. } => &["choices"],
            K::Number { .. } | K::ScalarNumber { .. } | K::Range { .. } => {
                &["minimum", "maximum", "unit"]
            }
            K::PhysicalSource { .. } | K::ShortText { .. } => &["maximum_bytes"],
            K::StructuredSource => return Err(Refusal::Order),
        }),
        Configuration(v) => Shape::Object(match v {
            V::Bool(_) => &["Bool"],
            V::U64(_) => &["U64"],
            V::I64(_) => &["I64"],
            V::Text(_) => &["Text"],
            V::Quantity(_) => &["Quantity"],
            V::Structured(_) => &["Structured"],
            V::Unit(_) => &["Unit"],
            V::TemperatureDifference(_) => &["TemperatureDifference"],
        }),
        ConfigurationPayload(v) => Shape::Object(match v {
            V::Structured(_) => &["profile", "canonical_value"],
            V::Quantity(_) | V::Unit(_) | V::TemperatureDifference(_) => {
                &["canonical_value", "source"]
            }
            _ => return Err(Refusal::Order),
        }),
        _ => return Err(Refusal::Order),
    })
}
pub(super) fn child(v: Nested<'_>, i: usize) -> Result<Node<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Control(v) => match i {
            0 => s(&v.key),
            1 => n(Configuration(&v.value)),
            2 => n(ControlKind(&v.kind)),
            3 => v
                .interaction
                .as_ref()
                .map_or(Node::Null, |v| n(Interaction(v))),
            _ => return Err(Refusal::Order),
        },
        ControlKind(v) if i == 0 && !matches!(v, K::StructuredSource) => n(KindPayload(v)),
        KindPayload(v) => match v {
            K::BooleanChoice { choices } if i == 0 => n(StaticStrings(choices)),
            K::TextChoice { choices } if i == 0 => n(Strings(choices)),
            K::Number {
                minimum,
                maximum,
                unit,
            }
            | K::Range {
                minimum,
                maximum,
                unit,
            } => match i {
                0 => u(*minimum),
                1 => u(*maximum),
                2 => o(unit.as_deref()),
                _ => return Err(Refusal::Order),
            },
            K::ScalarNumber {
                minimum,
                maximum,
                unit,
            } => match i {
                0 => Node::I64(*minimum),
                1 => Node::I64(*maximum),
                2 => s(unit),
                _ => return Err(Refusal::Order),
            },
            K::PhysicalSource { maximum_bytes } | K::ShortText { maximum_bytes } if i == 0 => {
                u(*maximum_bytes)
            }
            _ => return Err(Refusal::Order),
        },
        Configuration(v) if i == 0 => match v {
            V::Bool(v) => Node::Bool(*v),
            V::U64(v) => n(HumanU64(*v)),
            V::I64(v) => n(HumanI64(*v)),
            V::Text(v) => s(v),
            _ => n(ConfigurationPayload(v)),
        },
        ConfigurationPayload(v) => match v {
            V::Structured(v) => match i {
                0 => s(v.profile().as_str()),
                1 => n(Bytes(v.canonical_value())),
                _ => return Err(Refusal::Order),
            },
            V::Quantity(v) => match i {
                0 => n(Bytes(v.canonical_value())),
                1 => s(v.source()),
                _ => return Err(Refusal::Order),
            },
            V::Unit(v) => match i {
                0 => n(Bytes(v.canonical_value())),
                1 => s(v.source()),
                _ => return Err(Refusal::Order),
            },
            V::TemperatureDifference(v) => match i {
                0 => n(Bytes(v.canonical_value())),
                1 => s(v.source()),
                _ => return Err(Refusal::Order),
            },
            _ => return Err(Refusal::Order),
        },
        _ => return Err(Refusal::Order),
    })
}
