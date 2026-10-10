//! Capture checked capsules before pure expression lowering. Source names are
//! inspection evidence; Play consumes the admitted bytes, including old snapshots.
use super::*;
use crate::{CanonicalStartupValue as V, CheckedExpressionType, ExpressionSyntax as E};
use conduit_core::{kind_id, quantity_role_info_id, StructuredInfoType};

type Types = BTreeMap<String, CheckedExpressionType>;
type Payloads = BTreeMap<(usize, usize), Vec<u8>>;

pub(super) fn bind(
    expression: &mut E,
    plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, V>,
    catalog: &ProfileCatalog,
) -> Result<(Types, Payloads), CanonicalExpansionDiagnostic> {
    let mut startup = crate::StartupCatalog::new();
    startup.physical = catalog.physical.clone();
    let bindings = plot
        .local_values
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                match value {
                    V::PlotParameter(parameter) => {
                        environment.get(parameter).unwrap_or(value).clone()
                    }
                    _ => value.clone(),
                },
            )
        })
        .chain(
            environment
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        )
        .collect::<BTreeMap<_, _>>();
    let mut types = Types::new();
    let mut payloads = Payloads::new();
    walk(expression, &bindings, &startup, &mut types, &mut payloads)?;
    Ok((types, payloads))
}
fn text(value: &E) -> Option<String> {
    match value {
        E::Atomic(value) => Some(value.text.clone()),
        E::Unary {
            operator: crate::UnaryOperator::Negate,
            operand,
            ..
        } => Some(format!("-{}", text(operand)?)),
        E::Binary {
            operator: crate::BinaryOperator::Divide,
            left,
            right,
            ..
        } if left.span().end + 1 == right.span().start => {
            Some(format!("{}/{}", text(left)?, text(right)?))
        }
        E::SemanticCall {
            kind, arguments, ..
        } => Some(format!(
            "{}({})",
            kind.text,
            arguments
                .iter()
                .map(text)
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        )),
        _ => None,
    }
}
fn shadowed(value: &E, bindings: &BTreeMap<String, V>) -> bool {
    match value {
        E::Atomic(name) => bindings.contains_key(&name.text),
        E::Binary { left, right, .. } => shadowed(left, bindings) || shadowed(right, bindings),
        E::Unary { operand, .. } => shadowed(operand, bindings),
        _ => false,
    }
}
fn walk(
    value: &mut E,
    bindings: &BTreeMap<String, V>,
    catalog: &crate::StartupCatalog,
    types: &mut Types,
    payloads: &mut Payloads,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let span = value.span();
    let authored = text(value);
    let bound = match value {
        E::Atomic(name) => bindings.get(&name.text).cloned(),
        _ => None,
    };
    let physical_bound = bound
        .as_ref()
        .is_some_and(|v| matches!(v, V::Quantity(_) | V::Unit(_) | V::TemperatureDifference(_)));
    let constructor = if let E::SemanticCall {
        kind, arguments, ..
    } = value
    {
        if let Some(role) = catalog.physical.quantities.get(&kind.text) {
            if arguments.len() != 2 {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    "physical constructor requires coordinate and Unit".into(),
                ));
            }
            let symbol = text(&arguments[1]);
            let unit = symbol
                .as_ref()
                .and_then(|symbol| match bindings.get(symbol) {
                    Some(V::Unit(value)) => Some(value.value()),
                    Some(_) => None,
                    None => catalog
                        .physical
                        .units
                        .get(symbol)
                        .copied()
                        .map(conduit_core::Unit::from_definition),
                });
            if let (Some(unit), Some(coordinate)) = (unit, text(&arguments[0])) {
                let evidence = format!("{}({}, {})", kind.text, coordinate, unit.symbol());
                let quantity =
                    conduit_core::Quantity::parse_with_unit_evidence(&evidence, unit, role.role)
                        .map_err(|reason| {
                            CanonicalExpansionDiagnostic::new(
                                "CND-FRM-046",
                                format!("invalid physical constructor: {reason:?}"),
                            )
                        })?;
                if quantity.family() != role.family {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-046",
                        "constructor Unit belongs to another family".into(),
                    ));
                }
                Some(V::Quantity(
                    conduit_core::QuantityConfigurationValue::new(quantity, evidence)
                        .expect("checked constructor evidence"),
                ))
            } else {
                return Err(CanonicalExpansionDiagnostic::new("CND-FRM-046","physical constructor requires concrete checked coordinate and Unit; forward a Quantity parameter after construction".into()));
            }
        } else {
            None
        }
    } else {
        None
    };
    let candidate = if physical_bound {
        bound
    } else if constructor.is_some() {
        constructor
    } else if bound.is_some() || !matches!(value, E::Atomic(_)) && shadowed(value, bindings) {
        None
    } else if let Some(source) = &authored {
        crate::physical_declarations::value::parse_value(source, None, catalog).map_err(
            |error| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "physical expression at {}:{} refused: {error:?}",
                        span.line, span.column
                    ),
                )
            },
        )?
    } else {
        None
    };
    if let Some(candidate) = candidate {
        let (kind, bytes) = match candidate {
            V::Quantity(value) => (
                quantity_role_info_id(value.value().family(), value.value().role()).map_err(
                    |reason| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-046",
                            format!("invalid family role: {reason:?}"),
                        )
                    },
                )?,
                value.value().encode().to_vec(),
            ),
            V::Unit(value) => (
                conduit_core::UNIT_INFO_ID.into(),
                value.value().encode().to_vec(),
            ),
            V::TemperatureDifference(value) => {
                let coordinate = value.value().storage_coordinate();
                (
                    quantity_role_info_id(coordinate.family(), coordinate.role()).map_err(
                        |reason| {
                            CanonicalExpansionDiagnostic::new(
                                "CND-FRM-046",
                                format!("invalid delta role: {reason:?}"),
                            )
                        },
                    )?,
                    coordinate.encode().to_vec(),
                )
            }
            _ => unreachable!("physical admission returns physical values"),
        };
        let ty = StructuredInfoType::leaf(kind_id(&kind)).map_err(|reason| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                format!("invalid physical leaf: {reason:?}"),
            )
        })?;
        crate::expression_program::validate_capsule_literal(&ty, &bytes).map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "physical literal capsule violates its exact family and role".into(),
            )
        })?;
        let source = authored.expect("physical candidate has source text");
        if types
            .get(&source)
            .is_some_and(|existing| existing != &CheckedExpressionType::Semantic(kind_id(&kind)))
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "one physical expression spelling has conflicting family identities".into(),
            ));
        }
        types.insert(
            source.clone(),
            CheckedExpressionType::Semantic(kind_id(&kind)),
        );
        payloads.insert((span.start, span.end), bytes);
        *value = E::Atomic(crate::SpannedText { text: source, span });
        return Ok(());
    }
    match value {
        E::Atomic(_) | E::Input(_) | E::TypedGlyphLiteral(_) => {}
        E::Projection { value, .. }
        | E::Unary { operand: value, .. }
        | E::Variant { payload: value, .. } => walk(value, bindings, catalog, types, payloads)?,
        E::Binary { left, right, .. } => {
            walk(left, bindings, catalog, types, payloads)?;
            walk(right, bindings, catalog, types, payloads)?
        }
        E::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            walk(condition, bindings, catalog, types, payloads)?;
            walk(when_true, bindings, catalog, types, payloads)?;
            walk(when_false, bindings, catalog, types, payloads)?
        }
        E::Tuple { values, .. }
        | E::Collection { values, .. }
        | E::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                walk(value, bindings, catalog, types, payloads)?
            }
        }
        E::Record { fields, .. } => {
            for field in fields {
                walk(&mut field.value, bindings, catalog, types, payloads)?
            }
        }
    }
    Ok(())
}
