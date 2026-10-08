//! Portable finite constraints over exact canonical Info encodings.
//!
//! Patterns are carried as checked deterministic automata. Source regex syntax
//! belongs to checking; Play never compiles a pattern or selects a Host regex
//! dialect.

use alloc::vec::Vec;
mod storage;
use core::cmp::Ordering;
use serde::{Deserialize, Serialize};
pub use storage::*;

use crate::{validate_primitive_info, KindId, PrimitiveInfoRefusal};

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
        minimum: Option<crate::Quantity>,
        maximum: Option<crate::Quantity>,
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
        if canonical.len() > self.maximum_bytes as usize {
            return Err(ValueConstraintRefusal::Oversize {
                actual: u32::try_from(canonical.len()).unwrap_or(u32::MAX),
                maximum: self.maximum_bytes,
            });
        }
        validate_primitive_info(self.value_kind.as_str(), canonical)
            .map_err(ValueConstraintRefusal::MalformedPrimitive)?;
        for constraint in &self.constraints {
            constraint.validate_value(self.value_kind.as_str(), canonical)?;
        }
        Ok(())
    }

    /// Canonical, versioned bytes used wherever this checked contract enters a
    /// larger semantic identity. Authored spelling and Host representation are
    /// deliberately absent.
    pub fn identity_bytes(&self) -> Vec<u8> {
        let mut canonical = b"conduit.value-contract@3\0".to_vec();
        push_bytes(&mut canonical, self.value_kind.as_str().as_bytes());
        push_u32(&mut canonical, self.maximum_bytes);
        push_u32(&mut canonical, self.constraints.len() as u32);
        for constraint in &self.constraints {
            constraint.push_identity(&mut canonical);
        }
        canonical
    }
}

impl ValueConstraint {
    fn push_identity(&self, canonical: &mut Vec<u8>) {
        match self {
            Self::ByteLength { minimum, maximum } => {
                canonical.push(0);
                push_u32(canonical, *minimum);
                push_u32(canonical, *maximum);
            }
            Self::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(1);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(2);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(6);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(3);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.encode());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.encode());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::FloatFinite => canonical.push(7),
            Self::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(8);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::CanonicalMembership { members, negated } => {
                canonical.push(4);
                canonical.push(u8::from(*negated));
                push_u32(canonical, members.len() as u32);
                for member in members {
                    push_bytes(canonical, member);
                }
            }
            Self::TextPattern {
                pattern,
                anchored_start,
                anchored_end,
                negated,
            } => {
                canonical.push(5);
                canonical.push(u8::from(*anchored_start));
                canonical.push(u8::from(*anchored_end));
                canonical.push(u8::from(*negated));
                push_u32(canonical, u32::from(pattern.start_state));
                push_u32(canonical, pattern.maximum_input_characters);
                push_u32(canonical, pattern.maximum_match_steps);
                push_u32(canonical, pattern.states.len() as u32);
                for state in &pattern.states {
                    canonical.push(u8::from(state.accepting));
                    push_u32(canonical, state.transitions.len() as u32);
                    for transition in &state.transitions {
                        push_u32(canonical, transition.first_scalar);
                        push_u32(canonical, transition.last_scalar);
                        push_u32(canonical, u32::from(transition.target_state));
                    }
                }
            }
        }
    }

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
        }
    }

    fn validate_definition(
        &self,
        value_kind: &str,
        maximum_bytes: u32,
    ) -> Result<(), ConstraintDefinitionError> {
        match self {
            Self::ByteLength { minimum, maximum }
                if minimum > maximum || *maximum > maximum_bytes =>
            {
                Err(ConstraintDefinitionError::InvalidByteRange)
            }
            Self::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } if open_interval_is_invalid(
                minimum.as_ref(),
                maximum.as_ref(),
                *minimum_endpoint,
                *maximum_endpoint,
                Ord::cmp,
            ) =>
            {
                Err(ConstraintDefinitionError::InvalidUnsignedRange)
            }
            Self::UnsignedRange { .. } if value_kind != crate::COUNT_INFO_ID => {
                Err(ConstraintDefinitionError::WrongConstraintKind)
            }
            Self::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } if open_interval_is_invalid(
                minimum.as_ref(),
                maximum.as_ref(),
                *minimum_endpoint,
                *maximum_endpoint,
                Ord::cmp,
            ) =>
            {
                Err(ConstraintDefinitionError::InvalidSignedRange)
            }
            Self::SignedRange { .. } if value_kind != crate::SCALAR_INFO_ID => {
                Err(ConstraintDefinitionError::WrongConstraintKind)
            }
            Self::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                let kind = crate::primitive_info_kind(value_kind)
                    .ok_or(ConstraintDefinitionError::WrongConstraintKind)?;
                let minimum = minimum
                    .as_ref()
                    .map(|value| crate::FixedInteger::decode(kind, value))
                    .transpose()
                    .map_err(|_| ConstraintDefinitionError::WrongConstraintKind)?;
                let maximum = maximum
                    .as_ref()
                    .map(|value| crate::FixedInteger::decode(kind, value))
                    .transpose()
                    .map_err(|_| ConstraintDefinitionError::WrongConstraintKind)?;
                if open_interval_is_invalid(
                    minimum.as_ref(),
                    maximum.as_ref(),
                    *minimum_endpoint,
                    *maximum_endpoint,
                    |left, right| {
                        fixed_integer_cmp(*left, *right).unwrap_or(core::cmp::Ordering::Equal)
                    },
                ) {
                    Err(ConstraintDefinitionError::InvalidFixedIntegerRange)
                } else {
                    Ok(())
                }
            }
            Self::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } if open_quantity_range_is_invalid(
                minimum.as_ref(),
                maximum.as_ref(),
                *minimum_endpoint,
                *maximum_endpoint,
            ) =>
            {
                Err(ConstraintDefinitionError::InvalidQuantityRange)
            }
            Self::QuantityRange {
                minimum, maximum, ..
            } if value_kind != crate::QUANTITY_INFO_ID
                && minimum.as_ref().or(maximum.as_ref()).is_some_and(|bound| {
                    crate::quantity_info_dimension(value_kind) != Some(bound.dimension())
                }) =>
            {
                Err(ConstraintDefinitionError::WrongConstraintKind)
            }
            Self::FloatFinite if !float_kind(value_kind) => {
                Err(ConstraintDefinitionError::WrongConstraintKind)
            }
            Self::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                if !float_kind(value_kind) {
                    return Err(ConstraintDefinitionError::WrongConstraintKind);
                }
                let minimum = minimum
                    .as_deref()
                    .map(|value| {
                        decode_float(value_kind, value)
                            .ok_or(ConstraintDefinitionError::InvalidFloatRange)
                    })
                    .transpose()?;
                let maximum = maximum
                    .as_deref()
                    .map(|value| {
                        decode_float(value_kind, value)
                            .ok_or(ConstraintDefinitionError::InvalidFloatRange)
                    })
                    .transpose()?;
                if minimum.as_ref().is_some_and(|value| value.is_nan())
                    || maximum.as_ref().is_some_and(|value| value.is_nan())
                    || open_interval_is_invalid(
                        minimum.as_ref(),
                        maximum.as_ref(),
                        *minimum_endpoint,
                        *maximum_endpoint,
                        |left, right| left.semantic_cmp(*right).unwrap_or(Ordering::Equal),
                    )
                {
                    Err(ConstraintDefinitionError::InvalidFloatRange)
                } else {
                    Ok(())
                }
            }
            Self::CanonicalMembership { members, .. } if members.is_empty() => {
                Err(ConstraintDefinitionError::EmptyMembership)
            }
            Self::CanonicalMembership { members, .. } if members.len() > MAX_MEMBERSHIP_VALUES => {
                Err(ConstraintDefinitionError::TooManyMembershipValues)
            }
            Self::CanonicalMembership { members, .. }
                if members
                    .iter()
                    .any(|member| member.len() > maximum_bytes as usize)
                    || members.iter().map(Vec::len).sum::<usize>() > MAX_MEMBERSHIP_BYTES =>
            {
                Err(ConstraintDefinitionError::MembershipBytesExceeded)
            }
            Self::CanonicalMembership { members, .. }
                if members.windows(2).any(|pair| pair[0] >= pair[1]) =>
            {
                Err(ConstraintDefinitionError::NonCanonicalMembership)
            }
            Self::CanonicalMembership { members, .. }
                if members
                    .iter()
                    .any(|member| validate_primitive_info(value_kind, member).is_err()) =>
            {
                Err(ConstraintDefinitionError::MalformedMembership)
            }
            Self::TextPattern { .. } if value_kind != crate::TEXT_INFO_ID => {
                Err(ConstraintDefinitionError::WrongConstraintKind)
            }
            Self::TextPattern { pattern, .. } => pattern.validate_definition(),
            _ => Ok(()),
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
            } => {
                let value = crate::Quantity::decode(canonical)
                    .map_err(|_| ValueConstraintRefusal::QuantityRange)?;
                let above_minimum = minimum.as_ref().is_none_or(|minimum| {
                    value
                        .compare(*minimum)
                        .is_ok_and(|order| lower_accepts(order, *minimum_endpoint))
                });
                let below_maximum = maximum.as_ref().is_none_or(|maximum| {
                    value
                        .compare(*maximum)
                        .is_ok_and(|order| upper_accepts(order, *maximum_endpoint))
                });
                (above_minimum && below_maximum)
                    .then_some(())
                    .ok_or(ValueConstraintRefusal::QuantityRange)
            }
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

fn push_u32(canonical: &mut Vec<u8>, value: u32) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(canonical: &mut Vec<u8>, value: &[u8]) {
    push_u32(canonical, value.len() as u32);
    canonical.extend_from_slice(value);
}

fn push_optional<T>(
    canonical: &mut Vec<u8>,
    value: Option<&T>,
    push: impl FnOnce(&mut Vec<u8>, &T),
) {
    canonical.push(u8::from(value.is_some()));
    if let Some(value) = value {
        push(canonical, value);
    }
}

fn open_interval_is_invalid<T>(
    minimum: Option<&T>,
    maximum: Option<&T>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
    compare: impl FnOnce(&T, &T) -> core::cmp::Ordering,
) -> bool {
    (minimum.is_none() && minimum_endpoint != IntervalEndpoint::Inclusive)
        || (maximum.is_none() && maximum_endpoint != IntervalEndpoint::Inclusive)
        || (minimum.is_none() && maximum.is_none())
        || minimum.zip(maximum).is_some_and(|(minimum, maximum)| {
            interval_is_empty(
                compare(minimum, maximum),
                minimum_endpoint,
                maximum_endpoint,
            )
        })
}

fn open_quantity_range_is_invalid(
    minimum: Option<&crate::Quantity>,
    maximum: Option<&crate::Quantity>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> bool {
    open_interval_is_invalid(
        minimum,
        maximum,
        minimum_endpoint,
        maximum_endpoint,
        |minimum, maximum| {
            minimum
                .compare(*maximum)
                .unwrap_or(core::cmp::Ordering::Equal)
        },
    ) || minimum
        .zip(maximum)
        .is_some_and(|(minimum, maximum)| minimum.dimension() != maximum.dimension())
}

fn interval_is_empty(
    endpoint_order: core::cmp::Ordering,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> bool {
    endpoint_order.is_gt()
        || (endpoint_order.is_eq()
            && (minimum_endpoint == IntervalEndpoint::Exclusive
                || maximum_endpoint == IntervalEndpoint::Exclusive))
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

impl CheckedTextPattern {
    pub fn new(
        states: Vec<TextPatternState>,
        start_state: u16,
        maximum_input_characters: u32,
        maximum_match_steps: u32,
    ) -> Result<Self, ConstraintDefinitionError> {
        let pattern = Self {
            states,
            start_state,
            maximum_input_characters,
            maximum_match_steps,
        };
        pattern.validate_definition()?;
        Ok(pattern)
    }

    fn validate_definition(&self) -> Result<(), ConstraintDefinitionError> {
        if self.states.is_empty() {
            return Err(ConstraintDefinitionError::EmptyPattern);
        }
        if self.states.len() > MAX_PATTERN_STATES {
            return Err(ConstraintDefinitionError::TooManyPatternStates);
        }
        if usize::from(self.start_state) >= self.states.len() {
            return Err(ConstraintDefinitionError::InvalidStartState);
        }
        let transition_count = self
            .states
            .iter()
            .map(|state| state.transitions.len())
            .sum::<usize>();
        if transition_count > MAX_PATTERN_TRANSITIONS {
            return Err(ConstraintDefinitionError::TooManyPatternTransitions);
        }
        let maximum_fanout = self
            .states
            .iter()
            .map(|state| state.transitions.len() as u32)
            .max()
            .unwrap_or(0);
        let required_steps = self
            .maximum_input_characters
            .checked_mul(maximum_fanout.max(1))
            .ok_or(ConstraintDefinitionError::MatchWorkExceeded)?;
        if self.maximum_match_steps < required_steps
            || self.maximum_match_steps > MAX_PATTERN_MATCH_STEPS
        {
            return Err(ConstraintDefinitionError::MatchWorkExceeded);
        }
        for state in &self.states {
            if state
                .transitions
                .windows(2)
                .any(|pair| pair[0].first_scalar >= pair[1].first_scalar)
            {
                return Err(ConstraintDefinitionError::NonCanonicalTransitions);
            }
            for (index, transition) in state.transitions.iter().enumerate() {
                if char::from_u32(transition.first_scalar).is_none()
                    || char::from_u32(transition.last_scalar).is_none()
                    || transition.first_scalar > transition.last_scalar
                {
                    return Err(ConstraintDefinitionError::InvalidTransitionRange);
                }
                if usize::from(transition.target_state) >= self.states.len() {
                    return Err(ConstraintDefinitionError::InvalidTransitionTarget);
                }
                if state.transitions[index + 1..].iter().any(|candidate| {
                    transition.first_scalar <= candidate.last_scalar
                        && candidate.first_scalar <= transition.last_scalar
                }) {
                    return Err(ConstraintDefinitionError::NondeterministicTransitions);
                }
            }
        }
        Ok(())
    }

    pub fn is_match(&self, input: &str) -> bool {
        let mut state = usize::from(self.start_state);
        let mut characters = 0_u32;
        let mut steps = 0_u32;
        for character in input.chars() {
            characters = characters.saturating_add(1);
            if characters > self.maximum_input_characters {
                return false;
            }
            let scalar = character as u32;
            let mut target = None;
            for transition in &self.states[state].transitions {
                steps = steps.saturating_add(1);
                if steps > self.maximum_match_steps {
                    return false;
                }
                if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                    target = Some(usize::from(transition.target_state));
                    break;
                }
            }
            let Some(next) = target else {
                return false;
            };
            state = next;
        }
        self.states[state].accepting
    }

    /// Returns whether any bounded substring satisfies this deterministic
    /// pattern. Search remains portable checked work: every attempted start
    /// shares the one admitted step budget carried by the pattern.
    pub fn has_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut steps = 0_u32;
        for (start, _) in input
            .char_indices()
            .chain(core::iter::once((input.len(), '\0')))
        {
            let mut state = usize::from(self.start_state);
            if self.states[state].accepting {
                return true;
            }
            for character in input[start..].chars() {
                let scalar = character as u32;
                let mut target = None;
                for transition in &self.states[state].transitions {
                    steps = steps.saturating_add(1);
                    if steps > self.maximum_match_steps {
                        return false;
                    }
                    if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                        target = Some(usize::from(transition.target_state));
                        break;
                    }
                }
                let Some(next) = target else {
                    break;
                };
                state = next;
                if self.states[state].accepting {
                    return true;
                }
            }
        }
        false
    }

    pub fn has_prefix_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut state = usize::from(self.start_state);
        if self.states[state].accepting {
            return true;
        }
        let mut steps = 0_u32;
        for character in input.chars() {
            let scalar = character as u32;
            let mut target = None;
            for transition in &self.states[state].transitions {
                steps = steps.saturating_add(1);
                if steps > self.maximum_match_steps {
                    return false;
                }
                if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                    target = Some(usize::from(transition.target_state));
                    break;
                }
            }
            let Some(next) = target else {
                return false;
            };
            state = next;
            if self.states[state].accepting {
                return true;
            }
        }
        false
    }

    pub fn has_suffix_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut steps = 0_u32;
        for offset in input
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(core::iter::once(input.len()))
        {
            let mut state = usize::from(self.start_state);
            for character in input[offset..].chars() {
                let scalar = character as u32;
                let mut target = None;
                for transition in &self.states[state].transitions {
                    steps = steps.saturating_add(1);
                    if steps > self.maximum_match_steps {
                        return false;
                    }
                    if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                        target = Some(usize::from(transition.target_state));
                        break;
                    }
                }
                let Some(next) = target else {
                    state = usize::MAX;
                    break;
                };
                state = next;
            }
            if state != usize::MAX && self.states[state].accepting {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn literal_ab() -> CheckedTextPattern {
        CheckedTextPattern::new(
            vec![
                TextPatternState {
                    accepting: false,
                    transitions: vec![TextPatternTransition {
                        first_scalar: 'a' as u32,
                        last_scalar: 'a' as u32,
                        target_state: 1,
                    }],
                },
                TextPatternState {
                    accepting: false,
                    transitions: vec![TextPatternTransition {
                        first_scalar: 'b' as u32,
                        last_scalar: 'b' as u32,
                        target_state: 2,
                    }],
                },
                TextPatternState {
                    accepting: true,
                    transitions: Vec::new(),
                },
            ],
            0,
            2,
            2,
        )
        .unwrap()
    }

    #[test]
    fn deterministic_text_pattern_is_full_match_and_work_bounded() {
        let pattern = literal_ab();
        assert!(pattern.is_match("ab"));
        assert!(!pattern.is_match("a"));
        assert!(!pattern.is_match("abc"));
        assert!(!pattern.is_match("xb"));
    }

    #[test]
    fn zero_byte_unit_and_exact_empty_text_remain_expressible() {
        let unit = CheckedValueContract::new(crate::kind_id(crate::UNIT_INFO_ID), 0, vec![])
            .expect("unit has an exact zero-byte canonical encoding");
        assert_eq!(unit.validate(&[]), Ok(()));

        let empty = CheckedTextPattern::new(
            vec![TextPatternState {
                accepting: true,
                transitions: vec![],
            }],
            0,
            0,
            0,
        )
        .expect("an exact empty-text language requires no matching work");
        assert!(empty.is_match(""));
        assert!(!empty.is_match("x"));
    }

    #[test]
    fn overlapping_transitions_refuse_as_nondeterministic() {
        let error = CheckedTextPattern::new(
            vec![TextPatternState {
                accepting: true,
                transitions: vec![
                    TextPatternTransition {
                        first_scalar: 'a' as u32,
                        last_scalar: 'z' as u32,
                        target_state: 0,
                    },
                    TextPatternTransition {
                        first_scalar: 'm' as u32,
                        last_scalar: 'q' as u32,
                        target_state: 0,
                    },
                ],
            }],
            0,
            8,
            16,
        )
        .unwrap_err();
        assert_eq!(
            error,
            ConstraintDefinitionError::NondeterministicTransitions
        );
    }

    #[test]
    fn exact_contract_distinguishes_structure_from_constraint_refusal() {
        let count = CheckedValueContract::new(
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(2),
                maximum: Some(4),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();
        assert_eq!(count.validate(&crate::encode_count(3)), Ok(()));
        assert_eq!(
            count.validate(&crate::encode_count(7)),
            Err(ValueConstraintRefusal::UnsignedRange)
        );
        assert!(matches!(
            count.validate(&[0]),
            Err(ValueConstraintRefusal::MalformedPrimitive(_))
        ));
    }

    #[test]
    fn signed_and_quantity_ranges_compare_semantic_values() {
        let scalar = CheckedValueContract::new(
            crate::kind_id(crate::SCALAR_INFO_ID),
            crate::SCALAR_ENCODED_LEN as u32,
            vec![ValueConstraint::SignedRange {
                minimum: Some(-2_000_000),
                maximum: Some(2_000_000),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();
        assert_eq!(scalar.validate(&crate::Scalar::ZERO.encode()), Ok(()));
        assert_eq!(
            scalar.validate(&crate::Scalar::from_raw_microunits(3_000_000).encode()),
            Err(ValueConstraintRefusal::SignedRange)
        );

        let distance = CheckedValueContract::new(
            crate::kind_id(crate::DISTANCE_INFO_ID),
            crate::QUANTITY_ENCODED_LEN as u32,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(crate::Quantity::new(1, crate::QuantityUnit::Meter)),
                maximum: Some(crate::Quantity::new(2, crate::QuantityUnit::Meter)),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();
        assert_eq!(
            distance.validate(&crate::Quantity::new(150, crate::QuantityUnit::Centimeter).encode()),
            Ok(())
        );
        assert_eq!(
            distance.validate(&crate::Quantity::new(3, crate::QuantityUnit::Meter).encode()),
            Err(ValueConstraintRefusal::QuantityRange)
        );
    }

    #[test]
    fn open_semantic_ranges_keep_finite_carrier_admission_separate() {
        let positive = CheckedValueContract::new(
            crate::kind_id(crate::SCALAR_INFO_ID),
            crate::SCALAR_ENCODED_LEN as u32,
            vec![ValueConstraint::SignedRange {
                minimum: Some(0),
                maximum: None,
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();

        assert_eq!(
            positive.validate(&crate::Scalar::from_raw_microunits(1).encode()),
            Ok(())
        );
        assert_eq!(
            positive.validate(&crate::Scalar::from_raw_microunits(-1).encode()),
            Err(ValueConstraintRefusal::SignedRange)
        );
        assert_eq!(
            positive.validate(&[0; crate::SCALAR_ENCODED_LEN + 1]),
            Err(ValueConstraintRefusal::Oversize {
                actual: (crate::SCALAR_ENCODED_LEN + 1) as u32,
                maximum: crate::SCALAR_ENCODED_LEN as u32,
            })
        );
    }

    #[test]
    fn semantic_openness_has_identity_distinct_from_carrier_extrema() {
        let contract = |maximum| {
            CheckedValueContract::new(
                crate::kind_id(crate::COUNT_INFO_ID),
                crate::COUNT_ENCODED_LEN as u32,
                vec![ValueConstraint::UnsignedRange {
                    minimum: Some(0),
                    maximum,
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            )
            .unwrap()
        };
        assert_ne!(
            contract(None).identity_bytes(),
            contract(Some(u64::MAX)).identity_bytes()
        );
    }

    #[test]
    fn exclusive_interval_endpoints_are_exact_and_empty_ranges_refuse() {
        let open_count = CheckedValueContract::new(
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(2),
                maximum: Some(4),
                minimum_endpoint: IntervalEndpoint::Exclusive,
                maximum_endpoint: IntervalEndpoint::Exclusive,
            }],
        )
        .unwrap();
        assert_eq!(
            open_count.validate(&crate::encode_count(2)),
            Err(ValueConstraintRefusal::UnsignedRange)
        );
        assert_eq!(open_count.validate(&crate::encode_count(3)), Ok(()));
        assert_eq!(
            open_count.validate(&crate::encode_count(4)),
            Err(ValueConstraintRefusal::UnsignedRange)
        );

        assert_eq!(
            CheckedValueContract::new(
                crate::kind_id(crate::COUNT_INFO_ID),
                crate::COUNT_ENCODED_LEN as u32,
                vec![ValueConstraint::UnsignedRange {
                    minimum: Some(2),
                    maximum: Some(2),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Exclusive,
                }],
            ),
            Err(ConstraintDefinitionError::InvalidUnsignedRange)
        );
    }

    #[test]
    fn quantity_range_refuses_a_mismatched_kind_or_dimension() {
        assert_eq!(
            CheckedValueContract::new(
                crate::kind_id(crate::DISTANCE_INFO_ID),
                crate::QUANTITY_ENCODED_LEN as u32,
                vec![ValueConstraint::QuantityRange {
                    minimum: Some(crate::Quantity::new(1, crate::QuantityUnit::Second)),
                    maximum: Some(crate::Quantity::new(2, crate::QuantityUnit::Second)),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            ),
            Err(ConstraintDefinitionError::WrongConstraintKind)
        );
        assert_eq!(
            CheckedValueContract::new(
                crate::kind_id(crate::QUANTITY_INFO_ID),
                crate::QUANTITY_ENCODED_LEN as u32,
                vec![ValueConstraint::QuantityRange {
                    minimum: Some(crate::Quantity::new(1, crate::QuantityUnit::Meter)),
                    maximum: Some(crate::Quantity::new(2, crate::QuantityUnit::Second)),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            ),
            Err(ConstraintDefinitionError::InvalidQuantityRange)
        );
    }

    #[test]
    fn text_contract_combines_bound_membership_and_pattern() {
        let contract = CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            2,
            vec![
                ValueConstraint::CanonicalMembership {
                    members: vec![b"ab".to_vec(), b"xy".to_vec()],
                    negated: false,
                },
                ValueConstraint::TextPattern {
                    pattern: literal_ab(),
                    anchored_start: true,
                    anchored_end: true,
                    negated: false,
                },
            ],
        )
        .unwrap();
        assert_eq!(contract.validate(b"ab"), Ok(()));
        assert_eq!(
            contract.validate(b"xy"),
            Err(ValueConstraintRefusal::TextPattern)
        );
        assert_eq!(
            contract.validate(b"zz"),
            Err(ValueConstraintRefusal::Membership)
        );
    }

    #[test]
    fn negated_membership_and_pattern_are_exact_checked_truth() {
        let positive_membership = CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            5,
            vec![ValueConstraint::CanonicalMembership {
                members: vec![b"admin".to_vec(), b"root".to_vec()],
                negated: false,
            }],
        )
        .unwrap();
        let negative_membership = CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            5,
            vec![ValueConstraint::CanonicalMembership {
                members: vec![b"admin".to_vec(), b"root".to_vec()],
                negated: true,
            }],
        )
        .unwrap();
        assert_eq!(negative_membership.validate(b"guest"), Ok(()));
        assert_eq!(
            negative_membership.validate(b"root"),
            Err(ValueConstraintRefusal::Membership)
        );
        assert_ne!(
            positive_membership.identity_bytes(),
            negative_membership.identity_bytes()
        );

        let positive_pattern = CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            2,
            vec![ValueConstraint::TextPattern {
                pattern: literal_ab(),
                anchored_start: true,
                anchored_end: true,
                negated: false,
            }],
        )
        .unwrap();
        let negative_pattern = CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            2,
            vec![ValueConstraint::TextPattern {
                pattern: literal_ab(),
                anchored_start: true,
                anchored_end: true,
                negated: true,
            }],
        )
        .unwrap();
        assert_eq!(negative_pattern.validate(b"xy"), Ok(()));
        assert_eq!(
            negative_pattern.validate(b"ab"),
            Err(ValueConstraintRefusal::TextPattern)
        );
        assert_ne!(
            positive_pattern.identity_bytes(),
            negative_pattern.identity_bytes()
        );
    }

    #[test]
    fn incompatible_and_malformed_constraints_refuse_before_play() {
        assert_eq!(
            CheckedValueContract::new(
                crate::kind_id(crate::TEXT_INFO_ID),
                8,
                vec![ValueConstraint::UnsignedRange {
                    minimum: Some(0),
                    maximum: Some(8),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            ),
            Err(ConstraintDefinitionError::WrongConstraintKind)
        );
        assert_eq!(
            CheckedValueContract::new(
                crate::kind_id(crate::COUNT_INFO_ID),
                crate::COUNT_ENCODED_LEN as u32,
                vec![ValueConstraint::CanonicalMembership {
                    members: vec![vec![0]],
                    negated: false,
                }],
            ),
            Err(ConstraintDefinitionError::MalformedMembership)
        );
    }
}
