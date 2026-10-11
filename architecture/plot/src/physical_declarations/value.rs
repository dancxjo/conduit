use crate::prelude::*;
use crate::{CanonicalStartupValue as V, StartupCatalog, SyntaxCheckError};
use conduit_core::*;

/// Checks one physical expression against an immutable admitted catalogue.
pub(crate) fn parse_value(
    source: &str,
    expected: Option<&StructuredInfoType>,
    catalog: &StartupCatalog,
) -> Result<Option<V>, SyntaxCheckError> {
    if expected.is_some_and(|ty| crate::authored_quantity::expected_role(ty).is_none()) {
        return Ok(None);
    }
    let text = source.trim();
    let physical = &catalog.physical;
    let invalid = |reason: String| SyntaxCheckError::QuantityLiteral(reason);
    let value = if let Some(definition) = physical.units.get(text) {
        let unit = Unit::from_definition(*definition);
        Some(V::Unit(
            UnitConfigurationValue::new(unit, text.into())
                .ok_or_else(|| invalid("unit source evidence differs from its capsule".into()))?,
        ))
    } else {
        let constructor = text
            .split_once('(')
            .and_then(|(name, body)| body.strip_suffix(')').map(|body| (name.trim(), body)));
        let candidate = if let Some((name, body)) = constructor {
            if let Some(role) = physical.quantities.get(name) {
                let (_, symbol) = body.split_once(',').ok_or_else(|| {
                    invalid("quantity constructor requires coordinate and Unit".into())
                })?;
                let definition = physical.units.get(symbol.trim()).ok_or_else(|| {
                    invalid("quantity constructor requires a checked Unit".into())
                })?;
                if definition.family() != role.family {
                    return Err(invalid(
                        "quantity constructor Unit belongs to another family".into(),
                    ));
                }
                Some((Unit::from_definition(*definition), role.role))
            } else {
                None
            }
        } else {
            physical
                .units
                .iter()
                .filter(|(symbol, _)| {
                    text.strip_suffix(symbol.as_str()).is_some_and(|number| {
                        !number.is_empty()
                            && number
                                .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+')
                            && number.chars().all(|c| {
                                c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')
                            })
                    })
                })
                .max_by_key(|(symbol, _)| symbol.len())
                .map(|(_, definition)| {
                    (
                        Unit::from_definition(*definition),
                        definition.declared_role(),
                    )
                })
        };
        if let Some((unit, role)) = candidate {
            let quantity = Quantity::parse_with_unit_evidence(text, unit, role)
                .map_err(|reason| invalid(format!("invalid quantity expression: {reason:?}")))?;
            Some(V::Quantity(
                QuantityConfigurationValue::new(quantity, text.into()).ok_or_else(|| {
                    invalid("quantity source evidence differs from its capsule".into())
                })?,
            ))
        } else {
            None
        }
    };
    if value.is_none()
        && text.starts_with(|c: char| c.is_ascii_digit() || c == '-')
        && expected
            .is_some_and(|ty| crate::authored_quantity::expected_role(ty) == Some("quantity"))
    {
        let suffix =
            text.trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'));
        if suffix.is_empty() {
            return Err(invalid("quantity requires a Unit suffix".into()));
        }
        let choices = physical
            .units
            .keys()
            .filter(|symbol| symbol.ends_with(suffix))
            .take(8)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let hint = if choices.is_empty() {
            String::new()
        } else {
            format!("; matching declared Units: {choices}")
        };
        return Err(invalid(format!(
            "quantity has an undeclared Unit suffix{hint}"
        )));
    }
    if let (Some(value), Some(expected)) = (&value, expected) {
        if crate::authored_quantity::bytes(value, expected).is_none() {
            let choices=physical.units.iter().filter(|(_,unit)|physical.quantities.values().any(|role|role.family==unit.family() && matches!(expected.shape(), StructuredInfoTypeShape::Leaf(kind) if *kind==role.leaf_kind))).map(|(name,_)|name.as_str()).collect::<Vec<_>>().join(", ");
            return Err(invalid(format!("physical value does not satisfy the declared quantity family and role; declared Units: {choices}")));
        }
    }
    Ok(value)
}

/// Adds source declarations to an immutable compilation/editor snapshot.
pub fn checked_physical_catalog_for_document(
    document: &crate::SyntaxDocument,
    base: &StartupCatalog,
) -> Result<StartupCatalog, crate::SyntaxCheckDiagnostic> {
    super::context::install(document, base)
}

pub fn parse_checked_physical_value(
    source: &str,
    expected: Option<&StructuredInfoType>,
    catalog: &StartupCatalog,
) -> Result<Option<V>, crate::CanonicalExpansionDiagnostic> {
    let builtin;
    let catalog = if catalog.physical_sources.is_empty() {
        builtin = super::context::install(
            &crate::parse_syntax_document(BUILTIN_PHYSICAL_SOURCE),
            catalog,
        )
        .map_err(|reason| {
            crate::CanonicalExpansionDiagnostic::new("CND-FRM-058", reason.message)
        })?;
        &builtin
    } else {
        catalog
    };
    parse_value(source, expected, catalog).map_err(|reason| {
        crate::CanonicalExpansionDiagnostic::new("CND-FRM-040", format!("{reason:?}"))
    })
}
