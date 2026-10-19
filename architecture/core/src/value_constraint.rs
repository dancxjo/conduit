//! Portable finite constraints over exact canonical Info encodings.
//!
//! Patterns are carried as checked deterministic automata. Source regex syntax
//! belongs to checking; Play never compiles a pattern or selects a Host regex
//! dialect.

use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

use crate::{validate_primitive_info, KindId, PrimitiveInfoRefusal};
mod decode;
mod definition;
mod identity;
mod mathematical_near;
mod near_quantity;
pub use mathematical_near::PreparedMathematicalNearValueContract;
mod quantity_profile;
mod text_pattern;

#[cfg(test)]
mod tests;
pub use near_quantity::NEAR_VALUE_CONSTRAINT_WORK;
pub use quantity_profile::QuantityProfileContractRefusal;

pub const MAX_VALUE_CONSTRAINTS: usize = 16;
pub const MAX_MEMBERSHIP_VALUES: usize = 64;
pub const MAX_MEMBERSHIP_BYTES: usize = 4_096;
pub const MAX_PATTERN_STATES: usize = 256;
pub const MAX_PATTERN_TRANSITIONS: usize = 1_024;
pub const MAX_PATTERN_MATCH_STEPS: u32 = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CheckedValueContract {
    pub value_kind: KindId,
    pub maximum_bytes: u32,
    pub constraints: Vec<ValueConstraint>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum IntervalEndpoint {
    Inclusive,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ValueConstraint {
    ByteLength {
        minimum: u32,
        maximum: u32,
    },
    UnsignedRange {
        minimum: Option<u64>,
        maximum: Option<u64>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
    SignedRange {
        minimum: Option<i64>,
        maximum: Option<i64>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
    FixedIntegerRange {
        minimum: Option<Vec<u8>>,
        maximum: Option<Vec<u8>>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
    QuantityRange {
        minimum: Option<Box<crate::Quantity>>,
        maximum: Option<Box<crate::Quantity>>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
    FloatFinite,
    FloatRange {
        minimum: Option<Vec<u8>>,
        maximum: Option<Vec<u8>>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
    CanonicalMembership {
        members: Vec<Vec<u8>>,
        negated: bool,
    },
    TextPattern {
        pattern: CheckedTextPattern,
        anchored_start: bool,
        anchored_end: bool,
        negated: bool,
    },
    /// Absolute numerical tolerance over the distinct retained evidence wire.
    NearQuantity {
        source_kind: KindId,
        limit: Box<crate::Quantity>,
        schema: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CheckedTextPattern {
    pub states: Vec<TextPatternState>,
    pub start_state: u16,
    pub maximum_input_characters: u32,
    pub maximum_match_steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TextPatternState {
    pub accepting: bool,
    pub transitions: Vec<TextPatternTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TextPatternTransition {
    pub first_scalar: u32,
    pub last_scalar: u32,
    pub target_state: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintDefinitionError {
    TooManyConstraints,
    NonCanonicalConstraintOrder,
    WrongConstraintKind,
    InvalidByteRange,
    InvalidUnsignedRange,
    InvalidSignedRange,
    InvalidFixedIntegerRange,
    InvalidQuantityRange,
    InvalidFloatRange,
    InvalidNearQuantity,
    EmptyMembership,
    TooManyMembershipValues,
    MembershipBytesExceeded,
    NonCanonicalMembership,
    MalformedMembership,
    EmptyPattern,
    TooManyPatternStates,
    TooManyPatternTransitions,
    InvalidStartState,
    InvalidTransitionRange,
    InvalidTransitionTarget,
    NondeterministicTransitions,
    NonCanonicalTransitions,
    MatchWorkExceeded,
    InvalidCanonicalEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueConstraintRefusal {
    Oversize { actual: u32, maximum: u32 },
    MalformedPrimitive(PrimitiveInfoRefusal),
    WrongConstraintKind,
    ByteLength,
    UnsignedRange,
    SignedRange,
    FixedIntegerRange,
    QuantityRange,
    FloatFinite,
    NearQuantity(crate::NearQuantityEvidenceRefusal),
    FloatRange,
    Membership,
    TextPattern,
}

impl CheckedValueContract {
    pub fn new(
        value_kind: KindId,
        maximum_bytes: u32,
        constraints: Vec<ValueConstraint>,
    ) -> Result<Self, ConstraintDefinitionError> {
        let contract = Self {
            value_kind,
            maximum_bytes,
            constraints,
        };
        contract.validate_definition()?;
        Ok(contract)
    }

    pub fn validate_definition(&self) -> Result<(), ConstraintDefinitionError> {
        if self.constraints.len() > MAX_VALUE_CONSTRAINTS {
            return Err(ConstraintDefinitionError::TooManyConstraints);
        }
        if self
            .constraints
            .windows(2)
            .any(|pair| pair[0].rank() >= pair[1].rank())
        {
            return Err(ConstraintDefinitionError::NonCanonicalConstraintOrder);
        }
        for constraint in &self.constraints {
            constraint.validate_definition(self.value_kind.as_str(), self.maximum_bytes)?;
        }
        Ok(())
    }

    pub fn validate(&self, canonical: &[u8]) -> Result<(), ValueConstraintRefusal> {
        quantity_profile::validate_contract(self, canonical)
    }
}

impl ValueConstraint {
    fn rank(&self) -> u8 {
        match self {
            Self::ByteLength { .. } => 0,
            Self::UnsignedRange { .. } => 1,
            Self::SignedRange { .. } => 2,
            Self::FixedIntegerRange { .. } => 3,
            Self::QuantityRange { .. } => 4,
            Self::FloatFinite => 5,
            Self::FloatRange { .. } => 6,
            Self::CanonicalMembership { .. } => 7,
            Self::TextPattern { .. } => 8,
            Self::NearQuantity { .. } => 9,
        }
    }

    fn validate_value(
        &self,
        value_kind: &str,
        canonical: &[u8],
    ) -> Result<(), ValueConstraintRefusal> {
        match self {
            Self::ByteLength { minimum, maximum } => {
                let length = canonical.len() as u32;
                (length >= *minimum && length <= *maximum)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::ByteLength)
            }
            Self::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                if value_kind != crate::COUNT_INFO_ID {
                    return Err(ValueConstraintRefusal::WrongConstraintKind);
                }
                let value = crate::decode_count(canonical)
                    .map_err(ValueConstraintRefusal::MalformedPrimitive)?;
                (minimum
                    .as_ref()
                    .is_none_or(|minimum| lower_accepts(value.cmp(minimum), *minimum_endpoint))
                    && maximum
                        .as_ref()
                        .is_none_or(|maximum| upper_accepts(value.cmp(maximum), *maximum_endpoint)))
                .then_some(())
                .ok_or(ValueConstraintRefusal::UnsignedRange)
            }
            Self::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                if value_kind != crate::SCALAR_INFO_ID {
                    return Err(ValueConstraintRefusal::WrongConstraintKind);
                }
                let value = crate::Scalar::decode(canonical)
                    .map_err(|_| ValueConstraintRefusal::SignedRange)?
                    .raw_microunits();
                (minimum
                    .as_ref()
                    .is_none_or(|minimum| lower_accepts(value.cmp(minimum), *minimum_endpoint))
                    && maximum
                        .as_ref()
                        .is_none_or(|maximum| upper_accepts(value.cmp(maximum), *maximum_endpoint)))
                .then_some(())
                .ok_or(ValueConstraintRefusal::SignedRange)
            }
            Self::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                let kind = crate::primitive_info_kind(value_kind)
                    .ok_or(ValueConstraintRefusal::WrongConstraintKind)?;
                let value = crate::FixedInteger::decode(kind, canonical)
                    .map_err(|_| ValueConstraintRefusal::FixedIntegerRange)?;
                let above = minimum.as_ref().is_none_or(|minimum| {
                    crate::FixedInteger::decode(kind, minimum).is_ok_and(|minimum| {
                        fixed_integer_cmp(value, minimum)
                            .is_some_and(|order| lower_accepts(order, *minimum_endpoint))
                    })
                });
                let below = maximum.as_ref().is_none_or(|maximum| {
                    crate::FixedInteger::decode(kind, maximum).is_ok_and(|maximum| {
                        fixed_integer_cmp(value, maximum)
                            .is_some_and(|order| upper_accepts(order, *maximum_endpoint))
                    })
                });
                (above && below)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::FixedIntegerRange)
            }
            Self::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => quantity_profile::validate_range(
                canonical,
                minimum.as_deref(),
                maximum.as_deref(),
                *minimum_endpoint,
                *maximum_endpoint,
            ),
            Self::NearQuantity {
                source_kind,
                limit,
                schema,
            } => near_quantity::validate(schema, source_kind, limit, canonical),
            Self::FloatFinite => decode_float(value_kind, canonical)
                .filter(|value| value.is_finite())
                .map(|_| ())
                .ok_or(ValueConstraintRefusal::FloatFinite),
            Self::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                let value = decode_float(value_kind, canonical)
                    .ok_or(ValueConstraintRefusal::FloatRange)?;
                if value.is_nan() {
                    return Err(ValueConstraintRefusal::FloatRange);
                }
                let above = minimum.as_deref().is_none_or(|minimum| {
                    decode_float(value_kind, minimum).is_some_and(|minimum| {
                        value
                            .semantic_cmp(minimum)
                            .is_some_and(|order| lower_accepts(order, *minimum_endpoint))
                    })
                });
                let below = maximum.as_deref().is_none_or(|maximum| {
                    decode_float(value_kind, maximum).is_some_and(|maximum| {
                        value
                            .semantic_cmp(maximum)
                            .is_some_and(|order| upper_accepts(order, *maximum_endpoint))
                    })
                });
                (above && below)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::FloatRange)
            }
            Self::CanonicalMembership { members, negated } => {
                (members.iter().any(|member| member.as_slice() == canonical) != *negated)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::Membership)
            }
            Self::TextPattern {
                pattern,
                anchored_start,
                anchored_end,
                negated,
            } => {
                if value_kind != crate::TEXT_INFO_ID {
                    return Err(ValueConstraintRefusal::WrongConstraintKind);
                }
                let text = core::str::from_utf8(canonical)
                    .map_err(|_| ValueConstraintRefusal::TextPattern)?;
                ((match (*anchored_start, *anchored_end) {
                    (false, false) => pattern.has_match(text),
                    (true, false) => pattern.has_prefix_match(text),
                    (false, true) => pattern.has_suffix_match(text),
                    (true, true) => pattern.is_match(text),
                }) != *negated)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::TextPattern)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FloatValue {
    F32(crate::IeeeF32),
    F64(crate::IeeeF64),
}

impl FloatValue {
    fn is_nan(self) -> bool {
        match self {
            Self::F32(value) => value.value().is_nan(),
            Self::F64(value) => value.value().is_nan(),
        }
    }

    fn is_finite(self) -> bool {
        match self {
            Self::F32(value) => value.is_finite(),
            Self::F64(value) => value.is_finite(),
        }
    }

    /// IEEE total order over exact bits. NaNs are rejected by ordered
    /// refinements before this point; signed zero remains ordered and distinct.
    fn semantic_cmp(self, other: Self) -> Option<Ordering> {
        match (self, other) {
            (Self::F32(left), Self::F32(right)) => Some(left.cmp(&right)),
            (Self::F64(left), Self::F64(right)) => Some(left.cmp(&right)),
            _ => None,
        }
    }
}

fn float_kind(value_kind: &str) -> bool {
    matches!(
        crate::primitive_info_kind(value_kind),
        Some(crate::PrimitiveInfoKind::F32 | crate::PrimitiveInfoKind::F64)
    )
}

fn decode_float(value_kind: &str, canonical: &[u8]) -> Option<FloatValue> {
    match crate::primitive_info_kind(value_kind)? {
        crate::PrimitiveInfoKind::F32 => crate::IeeeF32::decode(canonical).map(FloatValue::F32),
        crate::PrimitiveInfoKind::F64 => crate::IeeeF64::decode(canonical).map(FloatValue::F64),
        _ => None,
    }
}

fn fixed_integer_cmp(
    left: crate::FixedInteger,
    right: crate::FixedInteger,
) -> Option<core::cmp::Ordering> {
    if left.kind() != right.kind() {
        return None;
    }
    left.signed()
        .and_then(|left| right.signed().map(|right| left.cmp(&right)))
        .or_else(|_| {
            left.unsigned()
                .and_then(|left| right.unsigned().map(|right| left.cmp(&right)))
        })
        .ok()
}

fn lower_accepts(order: core::cmp::Ordering, endpoint: IntervalEndpoint) -> bool {
    order.is_gt() || (order.is_eq() && endpoint == IntervalEndpoint::Inclusive)
}

fn upper_accepts(order: core::cmp::Ordering, endpoint: IntervalEndpoint) -> bool {
    order.is_lt() || (order.is_eq() && endpoint == IntervalEndpoint::Inclusive)
}
