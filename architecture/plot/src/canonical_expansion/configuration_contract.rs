//! Canonical startup value parsing and finite rule validation for source and tools.
use crate::{
    CanonicalExpansionDiagnostic, CanonicalStartupValue, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};

/// Check the same canonical startup value consumed by source expansion.
pub fn validate_startup_configuration(
    field: &KindConfigurationField,
    value: CanonicalStartupValue,
) -> Result<ConfigurationValue, CanonicalExpansionDiagnostic> {
    let value = parse_configuration_value(&field.key, value, &field.rule)?;
    validate_configuration_value(field, &value)?;
    Ok(value)
}

/// Check a canonical configuration value against its exact finite field rule.
/// Name resolution and startup Fore type checking precede this rule in source.
pub fn validate_configuration_value(
    field: &KindConfigurationField,
    value: &ConfigurationValue,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let accepted = match (&field.rule, value) {
        (KindConfigurationRule::Any, ConfigurationValue::Structured(_)) => false,
        (KindConfigurationRule::Any, _) => true,
        (
            KindConfigurationRule::U64Range { minimum, maximum }
            | KindConfigurationRule::DurationMillis { minimum, maximum },
            ConfigurationValue::U64(value),
        ) => (*minimum..=*maximum).contains(value),
        (KindConfigurationRule::I64Range { minimum, maximum }, ConfigurationValue::I64(value)) => {
            (*minimum..=*maximum).contains(value)
        }
        (
            KindConfigurationRule::QuantityRange {
                minimum,
                maximum,
                canonical_unit,
            },
            ConfigurationValue::Quantity(value),
        ) => value
            .convert(*canonical_unit)
            .is_ok_and(|value| (*minimum..=*maximum).contains(&value.value())),
        (KindConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
            value.len() <= *maximum as usize
        }
        (KindConfigurationRule::TextOneOf { values }, ConfigurationValue::Text(value)) => {
            values.contains(value)
        }
        (KindConfigurationRule::Structured { profile }, ConfigurationValue::Structured(value)) => {
            value.profile() == profile
        }
        _ => false,
    };
    if accepted {
        Ok(())
    } else {
        Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-040",
            format!(
                "startup value for '{}' violates its primitive contract",
                field.key
            ),
        ))
    }
}

fn parse_configuration_value(
    name: &str,
    value: CanonicalStartupValue,
    rule: &KindConfigurationRule,
) -> Result<ConfigurationValue, CanonicalExpansionDiagnostic> {
    if let KindConfigurationRule::Structured { profile } = rule {
        let CanonicalStartupValue::Structured(value) = value else {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("structured startup value '{name}' remains unresolved"),
            ));
        };
        let actual_profile = value.value_type().profile().map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' has no finite profile"),
            )
        })?;
        if actual_profile.value_kind() != profile {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' violates its exact profile"),
            ));
        }
        let concrete = value.try_concrete().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("structured startup value '{name}' remains unresolved"),
            )
        })?;
        let canonical = concrete.canonical_bytes().map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' exceeds canonical bounds"),
            )
        })?;
        let structured =
            conduit_core::StructuredConfigurationValue::new(profile.clone(), canonical)
                .ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("structured startup value '{name}' exceeds configuration bounds"),
                    )
                })?;
        return Ok(ConfigurationValue::Structured(structured));
    }
    if matches!(rule, KindConfigurationRule::QuantityRange { .. }) {
        return match value {
            CanonicalStartupValue::Quantity(quantity) => Ok(ConfigurationValue::Quantity(quantity)),
            CanonicalStartupValue::Literal(literal) => {
                conduit_core::Quantity::parse_plot_literal(&literal)
                    .map(ConfigurationValue::Quantity)
                    .map_err(|_| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-041",
                            format!("primitive startup quantity '{name}' is invalid"),
                        )
                    })
            }
            _ => Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("startup value '{name}' remains unresolved"),
            )),
        };
    }
    if matches!(rule, KindConfigurationRule::DurationMillis { .. }) {
        if let CanonicalStartupValue::Quantity(quantity) = value {
            let milliseconds = quantity
                .convert(conduit_core::QuantityUnit::Millisecond)
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("primitive startup duration '{name}' is invalid or inexact"),
                    )
                })?;
            return u64::try_from(milliseconds.value())
                .map(ConfigurationValue::U64)
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("primitive startup duration '{name}' is negative or overflows"),
                    )
                });
        }
    }
    let CanonicalStartupValue::Literal(literal) = value else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            format!("startup value '{name}' remains unresolved"),
        ));
    };
    if matches!(rule, KindConfigurationRule::DurationMillis { .. }) {
        parse_duration_millis(&literal)
            .map(ConfigurationValue::U64)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!("primitive startup duration '{name}' is invalid or overflows"),
                )
            })
    } else if matches!(rule, KindConfigurationRule::I64Range { .. }) {
        parse_scalar_configuration(&literal)
            .map(ConfigurationValue::I64)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!("primitive startup scalar '{name}' is invalid or overflows"),
                )
            })
    } else if literal == "true" || literal == "false" {
        Ok(ConfigurationValue::Bool(literal == "true"))
    } else if let Ok(value) = literal.parse::<u64>() {
        Ok(ConfigurationValue::U64(value))
    } else if let Some(value) = crate::text_value::parse_quoted_text(&literal) {
        Ok(ConfigurationValue::Text(value))
    } else {
        Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            format!("primitive startup value '{name}' cannot be represented by the current planner contract"),
        ))
    }
}

fn parse_scalar_configuration(literal: &str) -> Option<i64> {
    if !literal.contains('.') {
        return literal.parse().ok();
    }
    let (negative, magnitude) = literal
        .strip_prefix('-')
        .map_or((false, literal), |value| (true, value));
    let (whole, fraction) = magnitude.split_once('.')?;
    if whole.is_empty()
        || fraction.is_empty()
        || fraction.len() > 6
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.parse::<u64>().ok()?;
    let fraction_digits = fraction.len();
    let fraction = fraction.parse::<u64>().ok()?;
    let magnitude = whole
        .checked_mul(conduit_core::Scalar::SCALE as u64)?
        .checked_add(fraction.checked_mul(10_u64.pow((6 - fraction_digits) as u32))?)?;
    if negative {
        if magnitude == i64::MAX as u64 + 1 {
            Some(i64::MIN)
        } else {
            i64::try_from(magnitude).ok()?.checked_neg()
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}

fn parse_duration_millis(literal: &str) -> Option<u64> {
    let (digits, multiplier) = literal
        .strip_suffix("ms")
        .map(|digits| (digits, 1))
        .or_else(|| literal.strip_suffix('s').map(|digits| (digits, 1_000)))?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
}

#[cfg(test)]
#[path = "configuration_contract_tests.rs"]
mod tests;
