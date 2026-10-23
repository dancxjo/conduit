use super::*;
use ValueConstraint as C;
fn fields(c: &ValueConstraint) -> &'static [&'static str] {
    match c {
        C::ByteLength { .. } => &["minimum", "maximum"],
        C::UnsignedRange { .. }
        | C::SignedRange { .. }
        | C::FixedIntegerRange { .. }
        | C::QuantityRange { .. }
        | C::FloatRange { .. } => &["minimum", "maximum", "minimum_endpoint", "maximum_endpoint"],
        C::CanonicalMembership { .. } => &["members", "negated"],
        C::TextPattern { .. } => &["pattern", "anchored_start", "anchored_end", "negated"],
        C::NearQuantity { .. } => &["source_kind", "limit", "schema"],
        C::FloatFinite => &[],
    }
}
pub(super) fn shape(v: Nested<'_>) -> Result<Shape<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Contract(_) => Shape::Object(&["value_kind", "maximum_bytes", "constraints"]),
        Constraints(v) => Shape::Sequence(v.len()),
        Constraint(c) => match c {
            C::FloatFinite => Shape::String("FloatFinite"),
            C::ByteLength { .. } => Shape::Object(&["ByteLength"]),
            C::UnsignedRange { .. } => Shape::Object(&["UnsignedRange"]),
            C::SignedRange { .. } => Shape::Object(&["SignedRange"]),
            C::FixedIntegerRange { .. } => Shape::Object(&["FixedIntegerRange"]),
            C::QuantityRange { .. } => Shape::Object(&["QuantityRange"]),
            C::FloatRange { .. } => Shape::Object(&["FloatRange"]),
            C::CanonicalMembership { .. } => Shape::Object(&["CanonicalMembership"]),
            C::TextPattern { .. } => Shape::Object(&["TextPattern"]),
            C::NearQuantity { .. } => Shape::Object(&["NearQuantity"]),
        },
        ConstraintPayload(c) => Shape::Object(fields(c)),
        Pattern(_) => Shape::Object(&[
            "states",
            "start_state",
            "maximum_input_characters",
            "maximum_match_steps",
        ]),
        States(v) => Shape::Sequence(v.len()),
        State(_) => Shape::Object(&["accepting", "transitions"]),
        Transitions(v) => Shape::Sequence(v.len()),
        Transition(_) => Shape::Object(&["first_scalar", "last_scalar", "target_state"]),
        _ => return Err(Refusal::Order),
    })
}
pub(super) fn child(v: Nested<'_>, i: usize) -> Result<Node<'_>, Refusal> {
    use Nested::*;
    Ok(match v {
        Contract(v) => match i {
            0 => s(v.value_kind.as_str()),
            1 => u(v.maximum_bytes),
            2 => n(Constraints(&v.constraints)),
            _ => return Err(Refusal::Order),
        },
        Constraints(v) => n(Constraint(v.get(i).ok_or(Refusal::Order)?)),
        Constraint(v) if i == 0 && !matches!(v, C::FloatFinite) => n(ConstraintPayload(v)),
        ConstraintPayload(c) => match c {
            C::ByteLength { minimum, maximum } => match i {
                0 => u(*minimum),
                1 => u(*maximum),
                _ => return Err(Refusal::Order),
            },
            C::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => match i {
                0 => minimum.map_or(Node::Null, Node::U64),
                1 => maximum.map_or(Node::Null, Node::U64),
                2 => end(*minimum_endpoint),
                3 => end(*maximum_endpoint),
                _ => return Err(Refusal::Order),
            },
            C::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => match i {
                0 => minimum.map_or(Node::Null, Node::I64),
                1 => maximum.map_or(Node::Null, Node::I64),
                2 => end(*minimum_endpoint),
                3 => end(*maximum_endpoint),
                _ => return Err(Refusal::Order),
            },
            C::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            }
            | C::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => match i {
                0 => minimum.as_deref().map_or(Node::Null, |v| n(Bytes(v))),
                1 => maximum.as_deref().map_or(Node::Null, |v| n(Bytes(v))),
                2 => end(*minimum_endpoint),
                3 => end(*maximum_endpoint),
                _ => return Err(Refusal::Order),
            },
            C::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => match i {
                0 => minimum.as_deref().map_or(Node::Null, |v| n(Quantity(v))),
                1 => maximum.as_deref().map_or(Node::Null, |v| n(Quantity(v))),
                2 => end(*minimum_endpoint),
                3 => end(*maximum_endpoint),
                _ => return Err(Refusal::Order),
            },
            C::CanonicalMembership { members, negated } => match i {
                0 => n(ByteLists(members)),
                1 => Node::Bool(*negated),
                _ => return Err(Refusal::Order),
            },
            C::TextPattern {
                pattern,
                anchored_start,
                anchored_end,
                negated,
            } => match i {
                0 => n(Pattern(pattern)),
                1 => Node::Bool(*anchored_start),
                2 => Node::Bool(*anchored_end),
                3 => Node::Bool(*negated),
                _ => return Err(Refusal::Order),
            },
            C::NearQuantity {
                source_kind,
                limit,
                schema,
            } => match i {
                0 => s(source_kind.as_str()),
                1 => n(Quantity(limit)),
                2 => n(Bytes(schema)),
                _ => return Err(Refusal::Order),
            },
            C::FloatFinite => return Err(Refusal::Order),
        },
        Pattern(v) => match i {
            0 => n(States(&v.states)),
            1 => u(v.start_state),
            2 => u(v.maximum_input_characters),
            3 => u(v.maximum_match_steps),
            _ => return Err(Refusal::Order),
        },
        States(v) => n(State(v.get(i).ok_or(Refusal::Order)?)),
        State(v) => match i {
            0 => Node::Bool(v.accepting),
            1 => n(Transitions(&v.transitions)),
            _ => return Err(Refusal::Order),
        },
        Transitions(v) => n(Transition(v.get(i).ok_or(Refusal::Order)?)),
        Transition(v) => match i {
            0 => u(v.first_scalar),
            1 => u(v.last_scalar),
            2 => u(v.target_state),
            _ => return Err(Refusal::Order),
        },
        _ => return Err(Refusal::Order),
    })
}
