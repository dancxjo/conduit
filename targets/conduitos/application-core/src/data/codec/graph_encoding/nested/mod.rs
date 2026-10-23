//! Borrowed schema resolver. No Serde replay and no retained model references.
use super::super::Refusal;
use super::{Node, Shape};
use alloc::{string::String, vec::Vec};
use conduit_core::*;
use conduit_human::*;
use patchbay_graph::{FaceControl, FaceControlKind, FaceInteraction};
mod constraints;
mod controls;
mod human;
#[derive(Clone, Copy)]
pub(crate) enum Nested<'a> {
    Contract(&'a CheckedValueContract),
    Constraints(&'a [ValueConstraint]),
    Constraint(&'a ValueConstraint),
    ConstraintPayload(&'a ValueConstraint),
    Pattern(&'a CheckedTextPattern),
    States(&'a [TextPatternState]),
    State(&'a TextPatternState),
    Transitions(&'a [TextPatternTransition]),
    Transition(&'a TextPatternTransition),
    Bytes(&'a [u8]),
    ByteLists(&'a [Vec<u8>]),
    Quantity(&'a Quantity),
    Unit(&'a Unit),
    Strings(&'a [String]),
    StaticStrings(&'a [&'static str]),
    Control(&'a FaceControl),
    ControlKind(&'a FaceControlKind),
    KindPayload(&'a FaceControlKind),
    Configuration(&'a ConfigurationValue),
    ConfigurationPayload(&'a ConfigurationValue),
    Interaction(&'a FaceInteraction),
    InteractionContract(&'a InteractionContract),
    Current(&'a InteractionCurrentState),
    Domain(&'a InteractionDomain),
    Options(&'a [InteractionOption]),
    Option(&'a InteractionOption),
    Values(&'a [InteractionValue]),
    Value(&'a InteractionValue),
    Availability(&'a OptionAvailability),
    Unavailable(&'a OptionAvailabilityUnavailable),
    Family(&'a InteractionFamily),
    FamilyPayload(&'a InteractionFamily),
    HumanU64(u64),
    HumanI64(i64),
}
fn n(value: Nested<'_>) -> Node<'_> {
    Node::Nested(value)
}
fn s(value: &str) -> Node<'_> {
    Node::Str(value)
}
fn u(value: impl Into<u64>) -> Node<'static> {
    Node::U64(value.into())
}
fn o(value: Option<&str>) -> Node<'_> {
    value.map_or(Node::Null, s)
}
fn end(value: IntervalEndpoint) -> Node<'static> {
    s(match value {
        IntervalEndpoint::Inclusive => "Inclusive",
        IntervalEndpoint::Exclusive => "Exclusive",
    })
}
impl<'a> Nested<'a> {
    pub(super) fn shape(self) -> Result<Shape<'a>, Refusal> {
        use Nested::*;
        Ok(match self {
            Bytes(v) => Shape::Sequence(v.len()),
            ByteLists(v) => Shape::Sequence(v.len()),
            Strings(v) => Shape::Sequence(v.len()),
            StaticStrings(v) => Shape::Sequence(v.len()),
            Quantity(_) => Shape::Sequence(QUANTITY_ENCODED_LEN),
            Unit(_) => Shape::Sequence(UNIT_ENCODED_LEN),
            HumanU64(v) if v > 9_007_199_254_740_991 => Shape::DecimalU64(v),
            HumanU64(v) => Shape::U64(v),
            HumanI64(v) if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v) => {
                Shape::DecimalI64(v)
            }
            HumanI64(v) => Shape::I64(v),
            Contract(_) | Constraints(_) | Constraint(_) | ConstraintPayload(_) | Pattern(_)
            | States(_) | State(_) | Transitions(_) | Transition(_) => {
                return constraints::shape(self);
            }
            Control(_)
            | ControlKind(_)
            | KindPayload(_)
            | Configuration(_)
            | ConfigurationPayload(_) => return controls::shape(self),
            _ => return human::shape(self),
        })
    }
    pub(super) fn child(self, index: usize) -> Result<Node<'a>, Refusal> {
        use Nested::*;
        Ok(match self {
            Bytes(v) => u(*v.get(index).ok_or(Refusal::Order)?),
            ByteLists(v) => n(Bytes(v.get(index).ok_or(Refusal::Order)?)),
            Strings(v) => s(v.get(index).ok_or(Refusal::Order)?),
            StaticStrings(v) => s(v.get(index).ok_or(Refusal::Order)?),
            Quantity(v) => u(*v.encode().get(index).ok_or(Refusal::Order)?),
            Unit(v) => u(*v.encode().get(index).ok_or(Refusal::Order)?),
            HumanU64(_) | HumanI64(_) => return Err(Refusal::Order),
            Contract(_) | Constraints(_) | Constraint(_) | ConstraintPayload(_) | Pattern(_)
            | States(_) | State(_) | Transitions(_) | Transition(_) => {
                return constraints::child(self, index);
            }
            Control(_)
            | ControlKind(_)
            | KindPayload(_)
            | Configuration(_)
            | ConfigurationPayload(_) => return controls::child(self, index),
            _ => return human::child(self, index),
        })
    }
}
