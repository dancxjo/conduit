//! Generated record field order is semantic sorted order, not authored order.
use super::*;
use InteractionFamily as F;
pub(super) fn shape(v: Nested<'_>) -> Result<Shape<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Interaction(_) => Shape::Object(&["contract", "state"]),
        InteractionContract(_) => Shape::Object(&["semantic_id", "contract_identity", "family"]),
        Current(_) => Shape::Object(&[
            "state_identity",
            "contract_identity",
            "revision",
            "domain",
            "current",
        ]),
        Domain(_) => Shape::Object(&["revision", "options"]),
        Options(v) => Shape::Sequence(v.len()),
        Option(_) => Shape::Object(&["identity", "value", "availability"]),
        Values(v) => Shape::Sequence(v.len()),
        Value(_) => Shape::Object(&["canonical_bytes", "value_kind"]),
        Availability(v) => match v {
            OptionAvailability::Available => Shape::String("Available"),
            OptionAvailability::Unavailable(_) => Shape::Object(&["Unavailable"]),
        },
        Unavailable(_) => Shape::Object(&["reason_code"]),
        Family(f) => match f {
            F::Activate => Shape::String("Activate"),
            F::Boolean => Shape::String("Boolean"),
            F::ChooseOne(_) => Shape::Object(&["ChooseOne"]),
            F::ChooseMany(_) => Shape::Object(&["ChooseMany"]),
            F::Scalar(_) => Shape::Object(&["Scalar"]),
            F::RelativeAdjustment(_) => Shape::Object(&["RelativeAdjustment"]),
            F::Text(_) => Shape::Object(&["Text"]),
            F::Structured(_) => Shape::Object(&["Structured"]),
        },
        FamilyPayload(f) => Shape::Object(match f {
            F::ChooseMany(_) => &[
                "maximum_options",
                "maximum_selections",
                "minimum_selections",
                "value_kind",
            ],
            F::ChooseOne(_) => &["maximum_options", "value_kind"],
            F::RelativeAdjustment(_) => &["granularity", "maximum_delta", "minimum_delta", "unit"],
            F::Scalar(_) => &[
                "granularity",
                "maximum",
                "maximum_bound",
                "minimum",
                "minimum_bound",
                "unit",
            ],
            F::Structured(_) => &["maximum_bytes", "type_digest", "value_kind"],
            F::Text(_) => &["allow_empty", "maximum_bytes"],
            _ => return Err(Refusal::Order),
        }),
        _ => return Err(Refusal::Order),
    })
}
fn bound(v: &BoundKind) -> Node<'static> {
    s(match v {
        BoundKind::Inclusive => "Inclusive",
        BoundKind::Exclusive => "Exclusive",
    })
}
pub(super) fn child(v: Nested<'_>, i: usize) -> Result<Node<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Interaction(v) => match i {
            0 => n(InteractionContract(&v.contract)),
            1 => n(Current(&v.state)),
            _ => return Err(Refusal::Order),
        },
        InteractionContract(v) => match i {
            0 => s(&v.semantic_id),
            1 => s(&v.contract_identity),
            2 => n(Family(&v.family)),
            _ => return Err(Refusal::Order),
        },
        Current(v) => match i {
            0 => s(&v.state_identity),
            1 => s(&v.contract_identity),
            2 => u(v.revision),
            3 => v.domain.as_ref().map_or(Node::Null, |v| n(Domain(v))),
            4 => n(Values(&v.current)),
            _ => return Err(Refusal::Order),
        },
        Domain(v) => match i {
            0 => u(v.revision),
            1 => n(Options(&v.options)),
            _ => return Err(Refusal::Order),
        },
        Options(v) => n(Option(v.get(i).ok_or(Refusal::Order)?)),
        Option(v) => match i {
            0 => s(&v.identity),
            1 => n(Value(&v.value)),
            2 => n(Availability(&v.availability)),
            _ => return Err(Refusal::Order),
        },
        Values(v) => n(Value(v.get(i).ok_or(Refusal::Order)?)),
        Value(v) => match i {
            0 => n(Bytes(v.bytes())),
            1 => s(v.kind()),
            _ => return Err(Refusal::Order),
        },
        Availability(OptionAvailability::Unavailable(v)) if i == 0 => n(Unavailable(v)),
        Unavailable(v) if i == 0 => s(v.reason_code()),
        Family(v) if i == 0 && !matches!(v, F::Activate | F::Boolean) => n(FamilyPayload(v)),
        FamilyPayload(v) => match v {
            F::ChooseMany(v) => match i {
                0 => u(*v.maximum_options()),
                1 => u(*v.maximum_selections()),
                2 => u(*v.minimum_selections()),
                3 => s(v.value_kind().get()),
                _ => return Err(Refusal::Order),
            },
            F::ChooseOne(v) => match i {
                0 => u(*v.maximum_options()),
                1 => s(v.value_kind().get()),
                _ => return Err(Refusal::Order),
            },
            F::RelativeAdjustment(v) => match i {
                0 => Node::I64(*v.granularity()),
                1 => Node::I64(*v.maximum_delta()),
                2 => Node::I64(*v.minimum_delta()),
                3 => n(Unit(v.unit())),
                _ => return Err(Refusal::Order),
            },
            F::Scalar(v) => match i {
                0 => Node::I64(*v.granularity()),
                1 => Node::I64(*v.maximum()),
                2 => bound(v.maximum_bound()),
                3 => Node::I64(*v.minimum()),
                4 => bound(v.minimum_bound()),
                5 => n(Unit(v.unit())),
                _ => return Err(Refusal::Order),
            },
            F::Structured(v) => match i {
                0 => u(*v.maximum_bytes()),
                1 => n(Bytes(v.type_digest().get())),
                2 => s(v.value_kind().get()),
                _ => return Err(Refusal::Order),
            },
            F::Text(v) => match i {
                0 => Node::Bool(*v.allow_empty()),
                1 => u(*v.maximum_bytes()),
                _ => return Err(Refusal::Order),
            },
            _ => return Err(Refusal::Order),
        },
        _ => return Err(Refusal::Order),
    })
}
