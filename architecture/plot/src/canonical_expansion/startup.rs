//! Exact startup environment binding and canonical accumulator encoding.
use crate::prelude::*;
use crate::{CanonicalExpansionDiagnostic, CanonicalStartupValue, CheckedCanonicalGear};
use alloc::collections::BTreeMap;

pub(super) fn bind_child_environment(
    gear: &CheckedCanonicalGear,
    parent: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<BTreeMap<String, CanonicalStartupValue>, CanonicalExpansionDiagnostic> {
    gear.startup_bindings
        .iter()
        .map(|binding| {
            let value = substitute(&binding.value, parent)?;
            Ok((binding.name.clone(), value))
        })
        .collect()
}

pub(super) fn substitute(
    value: &CanonicalStartupValue,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<CanonicalStartupValue, CanonicalExpansionDiagnostic> {
    match value {
        CanonicalStartupValue::Literal(_) | CanonicalStartupValue::Quantity(_) => Ok(value.clone()),
        CanonicalStartupValue::Structured(value) if value.try_concrete().is_some() => {
            Ok(CanonicalStartupValue::Structured(value.clone()))
        }
        CanonicalStartupValue::Structured(expected) if expected.parameter_name().is_some() => {
            let name = expected.parameter_name().expect("guarded parameter");
            let Some(CanonicalStartupValue::Structured(actual)) = environment.get(name) else {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-039",
                    format!("structured startup parameter '{name}' has no exact value"),
                ));
            };
            if actual.value_type() != expected.value_type() || actual.try_concrete().is_none() {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-039",
                    format!("structured startup parameter '{name}' differs from its exact type"),
                ));
            }
            Ok(CanonicalStartupValue::Structured(actual.clone()))
        }
        CanonicalStartupValue::Structured(_) => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            "structured startup value remains unresolved".into(),
        )),
        CanonicalStartupValue::PoolReference(pool) => environment
            .get(pool.as_str())
            .cloned()
            .map_or_else(|| Ok(value.clone()), Ok),
        CanonicalStartupValue::PlotParameter(name) => {
            environment.get(name).cloned().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-039",
                    format!("back references undeclared outer startup value '{name}'"),
                )
            })
        }
    }
}

pub(super) fn canonical_initial_bytes(
    kind: &str,
    value: &CanonicalStartupValue,
) -> Result<Vec<u8>, CanonicalExpansionDiagnostic> {
    let bytes = match value {
        CanonicalStartupValue::Structured(value) => value
            .try_concrete()
            .and_then(|value| value.canonical_bytes().ok()),
        CanonicalStartupValue::Quantity(value) => Some(value.encode().to_vec()),
        CanonicalStartupValue::Literal(literal) => match conduit_core::primitive_info_kind(kind) {
            Some(conduit_core::PrimitiveInfoKind::Unit) if literal == "unit" => Some(Vec::new()),
            Some(conduit_core::PrimitiveInfoKind::Bool) => match literal.as_str() {
                "true" => Some(conduit_core::InfoBool::TRUE.encode().to_vec()),
                "false" => Some(conduit_core::InfoBool::FALSE.encode().to_vec()),
                _ => None,
            },
            Some(conduit_core::PrimitiveInfoKind::Text) => {
                crate::text_value::parse_quoted_text(literal).map(String::into_bytes)
            }
            Some(conduit_core::PrimitiveInfoKind::Count)
            | Some(conduit_core::PrimitiveInfoKind::U64) => literal
                .parse::<u64>()
                .ok()
                .map(u64::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U8) => {
                literal.parse::<u8>().ok().map(|v| vec![v])
            }
            Some(conduit_core::PrimitiveInfoKind::U16) => literal
                .parse::<u16>()
                .ok()
                .map(u16::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U32) => literal
                .parse::<u32>()
                .ok()
                .map(u32::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U128) => literal
                .parse::<u128>()
                .ok()
                .map(u128::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I8) => {
                literal.parse::<i8>().ok().map(|v| vec![v as u8])
            }
            Some(conduit_core::PrimitiveInfoKind::I16) => literal
                .parse::<i16>()
                .ok()
                .map(i16::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I32) => literal
                .parse::<i32>()
                .ok()
                .map(i32::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I64) => literal
                .parse::<i64>()
                .ok()
                .map(i64::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I128) => literal
                .parse::<i128>()
                .ok()
                .map(i128::to_le_bytes)
                .map(Vec::from),
            _ => None,
        },
        CanonicalStartupValue::PlotParameter(_) | CanonicalStartupValue::PoolReference(_) => None,
    }
    .ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-064",
            "fold initial accumulator has no exact canonical encoding".into(),
        )
    })?;
    if conduit_core::primitive_info_kind(kind).is_some() {
        conduit_core::validate_primitive_info(kind, &bytes).map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-064",
                "fold initial accumulator differs from its exact Value contract".into(),
            )
        })?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExpressionSyntax, Span, SpannedText};
    use conduit_core::{kind_id, StructuredInfoType};

    fn value(schema: &str, parameter: bool) -> CanonicalStartupValue {
        let expected = StructuredInfoType::nominal(
            kind_id(schema),
            StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        )
        .unwrap();
        let expression = ExpressionSyntax::Atomic(SpannedText {
            text: if parameter { "request" } else { "7" }.into(),
            span: Span {
                start: 0,
                end: 7,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 8,
            },
        });
        CanonicalStartupValue::Structured(
            crate::structured_startup::check_structured_expression(
                &expression,
                &expected,
                &mut |atomic, _| {
                    Ok(if parameter {
                        CanonicalStartupValue::PlotParameter(atomic.text.clone())
                    } else {
                        CanonicalStartupValue::Literal(atomic.text.clone())
                    })
                },
            )
            .unwrap(),
        )
    }

    #[test]
    fn structured_parameter_requires_a_concrete_value_of_the_same_exact_type() {
        let expected = value("fixture/request@1", true);
        let actual = value("fixture/request@1", false);
        let mut environment = BTreeMap::from([("request".into(), actual.clone())]);
        assert_eq!(substitute(&expected, &environment).unwrap(), actual);
        environment.insert("request".into(), value("fixture/other-request@1", false));
        assert!(substitute(&expected, &environment).is_err());
        environment.insert("request".into(), expected.clone());
        assert!(substitute(&expected, &environment).is_err());
        assert!(substitute(&expected, &BTreeMap::new()).is_err());
    }
}
