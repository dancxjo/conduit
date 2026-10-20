//! Admission of finite constraint definitions before value validation.
use super::*;

impl ValueConstraint {
    pub(super) fn validate_definition(
        &self,
        value_kind: &str,
        maximum_bytes: u32,
    ) -> Result<(), ConstraintDefinitionError> {
        match self {
            Self::NearQuantity {
                source_kind,
                limit,
                schema,
            } => near_quantity::validate_definition(
                value_kind,
                maximum_bytes,
                schema,
                source_kind,
                limit,
            ),
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
                minimum.as_deref(),
                maximum.as_deref(),
                *minimum_endpoint,
                *maximum_endpoint,
            ) =>
            {
                Err(ConstraintDefinitionError::InvalidQuantityRange)
            }
            Self::QuantityRange {
                minimum, maximum, ..
            } if (value_kind != crate::QUANTITY_INFO_ID
                && crate::parse_quantity_role_info_id(value_kind).is_none())
                || minimum.iter().chain(maximum.iter()).any(|bound| {
                    crate::validate_primitive_info(value_kind, &bound.encode()).is_err()
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
