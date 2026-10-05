//! Canonical native leaf values decoded from authored structured startup literals.
use super::structured_diagnostic;
use crate::prelude::*;
use crate::{Span, SyntaxCheckDiagnostic};

pub(super) fn canonical_leaf_literal(
    kind: &str,
    literal: &str,
    span: Span,
) -> Result<Vec<u8>, SyntaxCheckDiagnostic> {
    if kind == conduit_core::QUANTITY_INFO_ID {
        return conduit_core::Quantity::parse_plot_literal(literal)
            .map(|quantity| quantity.encode().to_vec())
            .map_err(|refusal| {
                structured_diagnostic(
                    span,
                    &format!(
                        "literal '{literal}' is incompatible with exact leaf kind '{kind}': {refusal:?}"
                    ),
                )
            });
    }
    let canonical = match kind {
        "value/unit" if crate::text_value::parse_quoted_text(literal).as_deref() == Some("") => {
            Some(Vec::new())
        }
        "value/text" => crate::text_value::parse_quoted_text(literal).map(|text| text.into_bytes()),
        "value/count" => literal
            .parse::<u64>()
            .ok()
            .map(|value| conduit_core::encode_count(value).to_vec()),
        "value/bool" => match literal {
            "false" => Some(conduit_core::InfoBool::FALSE.encode().to_vec()),
            "true" => Some(conduit_core::InfoBool::TRUE.encode().to_vec()),
            _ => None,
        },
        "value/scalar" => parse_scalar_literal(literal).map(|value| value.encode().to_vec()),
        conduit_core::F32_INFO_ID | conduit_core::F64_INFO_ID => {
            crate::value_type::refinement::checked_float_literal(kind, literal).ok()
        }
        integer_kind
            if conduit_core::primitive_info_kind(integer_kind).is_some_and(|kind| {
                matches!(
                    kind,
                    conduit_core::PrimitiveInfoKind::U8
                        | conduit_core::PrimitiveInfoKind::U16
                        | conduit_core::PrimitiveInfoKind::U32
                        | conduit_core::PrimitiveInfoKind::U64
                        | conduit_core::PrimitiveInfoKind::U128
                        | conduit_core::PrimitiveInfoKind::I8
                        | conduit_core::PrimitiveInfoKind::I16
                        | conduit_core::PrimitiveInfoKind::I32
                        | conduit_core::PrimitiveInfoKind::I64
                        | conduit_core::PrimitiveInfoKind::I128
                )
            }) =>
        {
            crate::integer_literal::canonicalize(literal, integer_kind)
                .ok()
                .flatten()
                .and_then(|canonical| {
                    let kind = conduit_core::primitive_info_kind(integer_kind)?;
                    let value = if integer_kind.starts_with("value/i") {
                        conduit_core::FixedInteger::from_signed(kind, canonical.parse().ok()?)
                            .ok()?
                    } else {
                        conduit_core::FixedInteger::from_unsigned(kind, canonical.parse().ok()?)
                            .ok()?
                    };
                    let (bytes, length) = value.encode();
                    Some(bytes[..length].to_vec())
                })
        }
        _ if !matches!(
            kind,
            "value/unit" | "value/text" | "value/count" | "value/bool" | "value/scalar"
        ) =>
        {
            Some(literal.as_bytes().to_vec())
        }
        _ => None,
    };
    canonical.ok_or_else(|| {
        structured_diagnostic(
            span,
            &format!("literal '{literal}' is incompatible with exact leaf kind '{kind}'"),
        )
    })
}

pub(crate) fn parse_scalar_literal(value: &str) -> Option<conduit_core::Scalar> {
    let (negative, value) = value
        .strip_prefix('-')
        .map_or((false, value), |value| (true, value));
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.is_none_or(|digits| {
            !digits.is_empty()
                && digits.len() <= 6
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return None;
    }
    let whole = parse_decimal_magnitude(whole)?;
    let fraction_digits = fraction.unwrap_or_default();
    let fraction = if fraction_digits.is_empty() {
        0
    } else {
        parse_decimal_magnitude(fraction_digits)?
    };
    let magnitude = whole
        .checked_mul(conduit_core::Scalar::SCALE as u64)
        .and_then(|whole| {
            whole.checked_add(fraction * 10_u64.pow((6 - fraction_digits.len()) as u32))
        })?;
    let raw = if negative {
        if magnitude == (i64::MAX as u64) + 1 {
            i64::MIN
        } else {
            -i64::try_from(magnitude).ok()?
        }
    } else {
        i64::try_from(magnitude).ok()?
    };
    Some(conduit_core::Scalar::from_raw_microunits(raw))
}

fn parse_decimal_magnitude(value: &str) -> Option<u64> {
    value.bytes().try_fold(0_u64, |magnitude, digit| {
        magnitude
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))
    })
}

#[cfg(test)]
mod float_literal_tests {
    use super::*;

    #[test]
    fn structured_text_literals_decode_source_quotes_and_escapes_once() {
        let span = Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
        };
        assert_eq!(
            canonical_leaf_literal("value/text", r#""French""#, span).unwrap(),
            b"French"
        );
        assert_eq!(
            canonical_leaf_literal("value/text", r#""猫\n\"""#, span).unwrap(),
            "猫\n\"".as_bytes()
        );
        assert_eq!(
            canonical_leaf_literal("value/text", r#""""#, span).unwrap(),
            b""
        );
        assert!(canonical_leaf_literal("value/text", "French", span).is_err());
    }

    #[test]
    fn exact_float_literals_preserve_signed_zero_and_refuse_excess_precision() {
        let span = Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
        };
        assert_eq!(
            canonical_leaf_literal(conduit_core::F32_INFO_ID, "-0.0", span).unwrap(),
            conduit_core::IeeeF32::from(-0.0).encode()
        );
        assert_eq!(
            canonical_leaf_literal(conduit_core::F64_INFO_ID, "0.1", span).unwrap(),
            conduit_core::IeeeF64::from(0.1).encode()
        );
        assert!(canonical_leaf_literal(conduit_core::F32_INFO_ID, "0.1234567891", span).is_err());
        assert!(canonical_leaf_literal(conduit_core::F32_INFO_ID, "NaN", span).is_err());
    }
}
