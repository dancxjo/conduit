use crate::prelude::*;
use crate::SyntaxCheckDiagnostic;
use conduit_core::{IntervalEndpoint, KindId, PrimitiveInfoKind, ValueConstraint};

pub(crate) fn checked_refinements(
    refinements: &[crate::ValueRefinement],
    maximum_bytes: Option<u64>,
    value_kind: &KindId,
    bound_span: crate::Span,
) -> Result<(u32, Vec<ValueConstraint>), SyntaxCheckDiagnostic> {
    let maximum_bytes = maximum_bytes
        .or_else(|| intrinsic_maximum_bytes(value_kind.as_str()).map(u64::from))
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code: "CND-FRM-057",
            span: bound_span,
            message: "a refined value requires an exact finite byte bound".into(),
        })?;
    let maximum_bytes = u32::try_from(maximum_bytes).map_err(|_| SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span: bound_span,
        message: "pattern input bound exceeds portable matching limits".into(),
    })?;
    let mut constraints = refinements
        .iter()
        .map(|refinement| match refinement {
            crate::ValueRefinement::Finite { span } => {
                if !matches!(
                    conduit_core::primitive_info_kind(value_kind.as_str()),
                    Some(PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64)
                ) {
                    return Err(SyntaxCheckDiagnostic {
                        code: "CND-FRM-057",
                        span: *span,
                        message: "finite may refine only F32 or F64".into(),
                    });
                }
                Ok(ValueConstraint::FloatFinite)
            }
            crate::ValueRefinement::TextPattern {
                source,
                case_insensitive,
                anchored_start,
                anchored_end,
                negated,
                span,
            } => {
                if value_kind.as_str() != conduit_core::TEXT_INFO_ID {
                    return Err(SyntaxCheckDiagnostic {
                        code: "CND-FRM-057",
                        span: *span,
                        message: "the ~ and !~ relations may refine only canonical Text info"
                            .into(),
                    });
                }
                let mut expression = crate::parse_text_pattern(&source.text).map_err(|error| {
                    SyntaxCheckDiagnostic {
                        code: "CND-FRM-057",
                        span: source.span,
                        message: alloc::format!("invalid portable text pattern: {error:?}"),
                    }
                })?;
                if *case_insensitive {
                    expression = expression.ascii_case_insensitive();
                }
                expression
                    .compile_search(maximum_bytes)
                    .map(|pattern| ValueConstraint::TextPattern {
                        pattern,
                        anchored_start: *anchored_start,
                        anchored_end: *anchored_end,
                        negated: *negated,
                    })
                    .map_err(|error| SyntaxCheckDiagnostic {
                        code: "CND-FRM-057",
                        span: *span,
                        message: alloc::format!(
                            "portable text pattern exceeds checked bounds: {error:?}"
                        ),
                    })
            }
            crate::ValueRefinement::Range {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
                span,
            } => checked_range(
                value_kind,
                minimum,
                maximum,
                endpoint(*minimum_endpoint),
                endpoint(*maximum_endpoint),
                *span,
            ),
            crate::ValueRefinement::Membership {
                members,
                negated,
                span,
            } => {
                let mut canonical = members
                    .iter()
                    .map(|member| checked_member(value_kind, member))
                    .collect::<Result<Vec<_>, _>>()?;
                canonical.sort();
                if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err(SyntaxCheckDiagnostic {
                        code: "CND-FRM-057",
                        span: *span,
                        message: "the membership relation contains the same canonical value more than once".into(),
                    });
                }
                Ok(ValueConstraint::CanonicalMembership {
                    members: canonical,
                    negated: *negated,
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    constraints.sort_by_key(constraint_rank);
    Ok((maximum_bytes, constraints))
}

fn endpoint(source: crate::RefinementIntervalEndpoint) -> IntervalEndpoint {
    match source {
        crate::RefinementIntervalEndpoint::Inclusive => IntervalEndpoint::Inclusive,
        crate::RefinementIntervalEndpoint::Exclusive => IntervalEndpoint::Exclusive,
    }
}

fn checked_range(
    value_kind: &KindId,
    minimum: &Option<crate::SpannedText>,
    maximum: &Option<crate::SpannedText>,
    mut minimum_endpoint: IntervalEndpoint,
    mut maximum_endpoint: IntervalEndpoint,
    span: crate::Span,
) -> Result<ValueConstraint, SyntaxCheckDiagnostic> {
    let invalid = |message: alloc::string::String| SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span,
        message,
    };
    match value_kind.as_str() {
        conduit_core::COUNT_INFO_ID => {
            let minimum = minimum
                .as_ref()
                .map(|minimum| {
                    minimum.text.parse().map_err(|_| {
                        invalid("range minimum is not an exact canonical Count literal".into())
                    })
                })
                .transpose()?;
            let maximum = maximum
                .as_ref()
                .map(|maximum| {
                    maximum.text.parse().map_err(|_| {
                        invalid("range maximum is not an exact canonical Count literal".into())
                    })
                })
                .transpose()?;
            minimum_endpoint = endpoint_or_inclusive(minimum.is_some(), minimum_endpoint);
            maximum_endpoint = endpoint_or_inclusive(maximum.is_some(), maximum_endpoint);
            Ok(ValueConstraint::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            })
        }
        conduit_core::SCALAR_INFO_ID => {
            let minimum = minimum
                .as_ref()
                .map(|minimum| {
                    crate::structured_startup::parse_scalar_literal(&minimum.text)
                        .ok_or_else(|| {
                            invalid("range minimum is not an exact Scalar literal".into())
                        })
                        .map(|value| value.raw_microunits())
                })
                .transpose()?;
            let maximum = maximum
                .as_ref()
                .map(|maximum| {
                    crate::structured_startup::parse_scalar_literal(&maximum.text)
                        .ok_or_else(|| {
                            invalid("range maximum is not an exact Scalar literal".into())
                        })
                        .map(|value| value.raw_microunits())
                })
                .transpose()?;
            minimum_endpoint = endpoint_or_inclusive(minimum.is_some(), minimum_endpoint);
            maximum_endpoint = endpoint_or_inclusive(maximum.is_some(), maximum_endpoint);
            Ok(ValueConstraint::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            })
        }
        kind if conduit_core::primitive_info_kind(kind).is_some_and(|kind| {
            matches!(
                kind,
                PrimitiveInfoKind::U8
                    | PrimitiveInfoKind::U16
                    | PrimitiveInfoKind::U32
                    | PrimitiveInfoKind::U64
                    | PrimitiveInfoKind::U128
                    | PrimitiveInfoKind::I8
                    | PrimitiveInfoKind::I16
                    | PrimitiveInfoKind::I32
                    | PrimitiveInfoKind::I64
                    | PrimitiveInfoKind::I128
            )
        }) =>
        {
            let kind = conduit_core::primitive_info_kind(kind).expect("matched integer kind");
            let minimum = minimum
                .as_ref()
                .map(|value| {
                    checked_integer_member(kind, value_kind.as_str(), &value.text).ok_or_else(
                        || invalid("range minimum is outside its exact integer Type".into()),
                    )
                })
                .transpose()?;
            let maximum = maximum
                .as_ref()
                .map(|value| {
                    checked_integer_member(kind, value_kind.as_str(), &value.text).ok_or_else(
                        || invalid("range maximum is outside its exact integer Type".into()),
                    )
                })
                .transpose()?;
            minimum_endpoint = endpoint_or_inclusive(minimum.is_some(), minimum_endpoint);
            maximum_endpoint = endpoint_or_inclusive(maximum.is_some(), maximum_endpoint);
            Ok(ValueConstraint::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            })
        }
        kind if kind == conduit_core::QUANTITY_INFO_ID
            || conduit_core::quantity_info_dimension(kind).is_some() =>
        {
            let minimum = minimum
                .as_ref()
                .map(|minimum| {
                    conduit_core::Quantity::parse_form_literal(&minimum.text).map_err(|error| {
                        invalid(alloc::format!("invalid range minimum: {error:?}"))
                    })
                })
                .transpose()?;
            let maximum = maximum
                .as_ref()
                .map(|maximum| {
                    conduit_core::Quantity::parse_form_literal(&maximum.text).map_err(|error| {
                        invalid(alloc::format!("invalid range maximum: {error:?}"))
                    })
                })
                .transpose()?;
            minimum_endpoint = endpoint_or_inclusive(minimum.is_some(), minimum_endpoint);
            maximum_endpoint = endpoint_or_inclusive(maximum.is_some(), maximum_endpoint);
            Ok(ValueConstraint::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            })
        }
        kind if matches!(
            conduit_core::primitive_info_kind(kind),
            Some(PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64)
        ) =>
        {
            let minimum = minimum
                .as_ref()
                .map(|value| checked_float_literal(value_kind.as_str(), &value.text))
                .transpose()
                .map_err(|_| {
                    invalid("range minimum is not an exact finite float literal".into())
                })?;
            let maximum = maximum
                .as_ref()
                .map(|value| checked_float_literal(value_kind.as_str(), &value.text))
                .transpose()
                .map_err(|_| {
                    invalid("range maximum is not an exact finite float literal".into())
                })?;
            minimum_endpoint = endpoint_or_inclusive(minimum.is_some(), minimum_endpoint);
            maximum_endpoint = endpoint_or_inclusive(maximum.is_some(), maximum_endpoint);
            Ok(ValueConstraint::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            })
        }
        _ => Err(invalid(
            "a range relation requires Count, Scalar, or exact semantic Quantity info".into(),
        )),
    }
}

fn endpoint_or_inclusive(present: bool, endpoint: IntervalEndpoint) -> IntervalEndpoint {
    if present {
        endpoint
    } else {
        IntervalEndpoint::Inclusive
    }
}

fn checked_member(
    value_kind: &KindId,
    member: &crate::SpannedText,
) -> Result<Vec<u8>, SyntaxCheckDiagnostic> {
    let invalid = |detail: &str| SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span: member.span,
        message: alloc::format!(
            "member '{}' is not exact canonical {} info: {detail}",
            member.text,
            value_kind.as_str()
        ),
    };
    let bytes = match conduit_core::primitive_info_kind(value_kind.as_str()) {
        Some(PrimitiveInfoKind::Unit) if member.text == "()" => Vec::new(),
        Some(PrimitiveInfoKind::Bool) => match member.text.as_str() {
            "false" => conduit_core::InfoBool::FALSE.encode().to_vec(),
            "true" => conduit_core::InfoBool::TRUE.encode().to_vec(),
            _ => return Err(invalid("expected true or false")),
        },
        Some(PrimitiveInfoKind::Count) => member
            .text
            .parse::<u64>()
            .map(conduit_core::encode_count)
            .map(|value| value.to_vec())
            .map_err(|_| invalid("expected a nonnegative decimal integer"))?,
        Some(PrimitiveInfoKind::Scalar) => {
            crate::structured_startup::parse_scalar_literal(&member.text)
                .map(|value| value.encode().to_vec())
                .ok_or_else(|| invalid("expected an exact six-decimal Scalar"))?
        }
        Some(PrimitiveInfoKind::Text) => crate::text_value::parse_quoted_text(&member.text)
            .map(|value| value.into_bytes())
            .ok_or_else(|| invalid("expected a quoted Text literal"))?,
        Some(
            PrimitiveInfoKind::Quantity
            | PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount,
        ) => conduit_core::Quantity::parse_form_literal(&member.text)
            .map(|value| value.encode().to_vec())
            .map_err(|_| invalid("expected an exact semantic Quantity literal"))?,
        Some(
            kind @ (PrimitiveInfoKind::U8
            | PrimitiveInfoKind::U16
            | PrimitiveInfoKind::U32
            | PrimitiveInfoKind::U64
            | PrimitiveInfoKind::U128
            | PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128),
        ) => checked_integer_member(kind, value_kind.as_str(), &member.text)
            .ok_or_else(|| invalid("expected an in-range exact-width integer"))?,
        Some(PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64) => {
            checked_float_literal(value_kind.as_str(), &member.text)
                .map_err(|_| invalid("expected an exact finite float literal"))?
        }
        _ => return Err(invalid("this Info kind has no admitted literal grammar")),
    };
    conduit_core::validate_primitive_info(value_kind.as_str(), &bytes)
        .map_err(|error| invalid(&alloc::format!("{error:?}")))?;
    Ok(bytes)
}

pub(crate) fn checked_float_literal(value_kind: &str, literal: &str) -> Result<Vec<u8>, ()> {
    let mantissa = literal
        .split_once(['e', 'E'])
        .map_or(literal, |(mantissa, _)| mantissa);
    let significant_digits = mantissa
        .bytes()
        .filter(|byte| byte.is_ascii_digit())
        .skip_while(|byte| *byte == b'0')
        .count()
        .max(1);
    if significant_digits > 17 {
        return Err(());
    }
    match conduit_core::primitive_info_kind(value_kind) {
        Some(PrimitiveInfoKind::F32) => {
            if significant_digits > 9 {
                return Err(());
            }
            let value = literal.parse::<f32>().map_err(|_| ())?;
            value
                .is_finite()
                .then(|| conduit_core::IeeeF32::from(value).encode().to_vec())
                .ok_or(())
        }
        Some(PrimitiveInfoKind::F64) => {
            let value = literal.parse::<f64>().map_err(|_| ())?;
            value
                .is_finite()
                .then(|| conduit_core::IeeeF64::from(value).encode().to_vec())
                .ok_or(())
        }
        _ => Err(()),
    }
}

fn checked_integer_member(
    kind: PrimitiveInfoKind,
    value_kind: &str,
    literal: &str,
) -> Option<Vec<u8>> {
    let canonical = crate::integer_literal::canonicalize(literal, value_kind)
        .ok()
        .flatten()?;
    let value = if matches!(
        kind,
        PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    ) {
        conduit_core::FixedInteger::from_signed(kind, canonical.parse().ok()?).ok()?
    } else {
        conduit_core::FixedInteger::from_unsigned(kind, canonical.parse().ok()?).ok()?
    };
    let (bytes, length) = value.encode();
    Some(bytes[..length].to_vec())
}

fn intrinsic_maximum_bytes(value_kind: &str) -> Option<u32> {
    match conduit_core::primitive_info_kind(value_kind)? {
        PrimitiveInfoKind::Unit | PrimitiveInfoKind::CancellationRequest => Some(0),
        PrimitiveInfoKind::Bool => Some(conduit_core::BOOL_ENCODED_LEN as u32),
        PrimitiveInfoKind::Count => Some(conduit_core::COUNT_ENCODED_LEN as u32),
        PrimitiveInfoKind::Scalar => Some(conduit_core::SCALAR_ENCODED_LEN as u32),
        PrimitiveInfoKind::Quantity
        | PrimitiveInfoKind::Distance
        | PrimitiveInfoKind::Frequency
        | PrimitiveInfoKind::Duration
        | PrimitiveInfoKind::Voltage
        | PrimitiveInfoKind::Temperature
        | PrimitiveInfoKind::Angle
        | PrimitiveInfoKind::Ratio
        | PrimitiveInfoKind::PixelCount => Some(conduit_core::QUANTITY_ENCODED_LEN as u32),
        PrimitiveInfoKind::QuantityUnit => Some(conduit_core::QUANTITY_UNIT_ENCODED_LEN as u32),
        kind @ (PrimitiveInfoKind::U8
        | PrimitiveInfoKind::U16
        | PrimitiveInfoKind::U32
        | PrimitiveInfoKind::U64
        | PrimitiveInfoKind::U128
        | PrimitiveInfoKind::I8
        | PrimitiveInfoKind::I16
        | PrimitiveInfoKind::I32
        | PrimitiveInfoKind::I64
        | PrimitiveInfoKind::I128) => Some(conduit_core::fixed_integer_bytes(kind) as u32),
        PrimitiveInfoKind::F32 => Some(conduit_core::IeeeF32::ENCODED_LEN as u32),
        PrimitiveInfoKind::F64 => Some(conduit_core::IeeeF64::ENCODED_LEN as u32),
        PrimitiveInfoKind::Text | PrimitiveInfoKind::Bytes | PrimitiveInfoKind::Terminal => None,
    }
}

fn constraint_rank(constraint: &ValueConstraint) -> u8 {
    match constraint {
        ValueConstraint::ByteLength { .. } => 0,
        ValueConstraint::UnsignedRange { .. } => 1,
        ValueConstraint::SignedRange { .. } => 2,
        ValueConstraint::FixedIntegerRange { .. } => 3,
        ValueConstraint::QuantityRange { .. } => 4,
        ValueConstraint::FloatFinite => 5,
        ValueConstraint::FloatRange { .. } => 6,
        ValueConstraint::CanonicalMembership { .. } => 7,
        ValueConstraint::TextPattern { .. } => 8,
    }
}
