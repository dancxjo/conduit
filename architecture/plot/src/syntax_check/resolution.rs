use crate::prelude::*;
use crate::{CanonicalStartupValue, SyntaxCheckError};
use alloc::collections::{BTreeMap, BTreeSet};

pub(crate) struct Resolver<'a> {
    catalog: &'a crate::StartupCatalog,
    pub(crate) locals: BTreeMap<String, &'a crate::LocalValue>,
    parameters: BTreeMap<String, conduit_core::KindId>,
    context_budget: Option<(usize, usize)>,
    runtime_ports: BTreeSet<String>,
    pools: BTreeSet<String>,
    resolved: BTreeMap<String, CanonicalStartupValue>,
    visiting: BTreeSet<String>,
    source_values:
        &'a BTreeMap<(usize, usize), (crate::Expression, crate::CanonicalStructuredStartupValue)>,
    prepared_glyphs: &'a BTreeMap<
        (usize, usize),
        (
            crate::TypedGlyphLiteralSyntax,
            crate::CanonicalStructuredStartupValue,
        ),
    >,
}

impl<'a> Resolver<'a> {
    pub(crate) fn new(
        locals: BTreeMap<String, &'a crate::LocalValue>,
        parameters: BTreeMap<String, conduit_core::KindId>,
        runtime_ports: BTreeSet<String>,
        pools: BTreeSet<String>,
        catalog: &'a crate::StartupCatalog,
    ) -> Self {
        Self {
            catalog,
            context_budget: None,
            prepared_glyphs: &catalog.prepared_glyph_values,
            source_values: &catalog.prepared_source_values,
            locals,
            parameters,
            runtime_ports,
            pools,
            resolved: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    pub(crate) fn bound_glyph_context(&mut self) {
        self.context_budget = Some((0, 0));
    }

    pub(crate) fn resolve_name(
        &mut self,
        name: &str,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if let Some(value) = self.resolved.get(name) {
            if expected
                .and_then(crate::authored_quantity::expected_role)
                .is_some()
                && crate::authored_quantity::value_kind(value).is_none()
            {
                let expression = self.locals[name].value.clone();
                return self.resolve_expression(&expression, expected);
            }
            if let (Some(expected), CanonicalStartupValue::Structured(actual)) = (expected, value) {
                if actual.value_type() != expected {
                    return Err(SyntaxCheckError::StructuredExpression(
                        format!(
                            "structured local '{name}' was already checked with an incompatible exact type"
                        ),
                        None,
                    ));
                }
            }
            return Ok(value.clone());
        }
        if self.visiting.len() >= 64 {
            return Err(SyntaxCheckError::StructuredExpression(
                "immutable Source dependency depth exceeds 64".into(),
                None,
            ));
        }
        if !self.visiting.insert(name.to_string()) {
            return Err(SyntaxCheckError::DependencyCycle(name.to_string()));
        }
        let expression = self.locals[name].value.clone();
        let value = self.resolve_expression(&expression, expected)?;
        self.visiting.remove(name);
        if let (Some((entries, bytes)), CanonicalStartupValue::Structured(structured)) =
            (&mut self.context_budget, &value)
        {
            let encoded = structured
                .try_concrete()
                .and_then(|value| value.canonical_bytes().ok());
            let size = encoded
                .ok_or_else(|| {
                    SyntaxCheckError::StructuredExpression(
                        "glyph Source context must be concrete and canonically bounded".into(),
                        Some(expression.span),
                    )
                })?
                .len();
            let retained_bytes = bytes
                .saturating_add(expression.text.len())
                .saturating_add(size);
            if *entries >= 64 || retained_bytes > 1024 * 1024 {
                return Err(SyntaxCheckError::StructuredExpression(
                    "glyph Source context exceeds 64 entries or 1 MiB of authored and canonical bytes".into(), Some(expression.span)));
            }
            *entries += 1;
            *bytes = retained_bytes;
        }
        self.resolved.insert(name.to_string(), value.clone());
        Ok(value)
    }

    pub(super) fn resolve_expression(
        &mut self,
        expression: &crate::Expression,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if let crate::ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } = &expression.syntax
        {
            if let Some(role) = self.catalog.physical.quantities.get(&kind.text).cloned() {
                if arguments.len() != 2 {
                    return Err(SyntaxCheckError::QuantityLiteral(
                        "quantity constructor requires coordinate and Unit".into(),
                    ));
                }
                let unit_syntax = &arguments[1];
                let start = unit_syntax
                    .span()
                    .start
                    .saturating_sub(expression.span.start);
                let end = unit_syntax.span().end.saturating_sub(expression.span.start);
                let unit_expression = crate::Expression {
                    text: expression
                        .text
                        .get(start..end)
                        .ok_or_else(|| {
                            SyntaxCheckError::QuantityLiteral(
                                "constructor operand source span is invalid".into(),
                            )
                        })?
                        .into(),
                    syntax: unit_syntax.clone(),
                    span: unit_syntax.span(),
                };
                let unit_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                    conduit_core::UNIT_INFO_ID,
                ))
                .expect("Unit leaf");
                let unit_value = self.resolve_expression(&unit_expression, Some(&unit_type))?;
                let CanonicalStartupValue::Unit(unit_value) = unit_value else {
                    return Err(SyntaxCheckError::QuantityLiteral(
                        "quantity constructor operand is not a concrete Unit".into(),
                    ));
                };
                let unit = unit_value.value();
                if unit.family() != role.family {
                    return Err(SyntaxCheckError::QuantityLiteral(
                        "quantity constructor Unit belongs to another family".into(),
                    ));
                }
                let number = &arguments[0];
                let start = number.span().start.saturating_sub(expression.span.start);
                let end = number.span().end.saturating_sub(expression.span.start);
                let coordinate = expression.text.get(start..end).ok_or_else(|| {
                    SyntaxCheckError::QuantityLiteral(
                        "constructor coordinate source span is invalid".into(),
                    )
                })?;
                let resolved_coordinate = if self.locals.contains_key(coordinate) {
                    match self.resolve_name(coordinate, None)? {
                        CanonicalStartupValue::Literal(value) => value,
                        _ => {
                            return Err(SyntaxCheckError::QuantityLiteral(
                                "quantity constructor coordinate is not a concrete decimal".into(),
                            ))
                        }
                    }
                } else {
                    coordinate.to_string()
                };
                let evidence = format!("{}({}, {})", kind.text, resolved_coordinate, unit.symbol());
                let quantity =
                    conduit_core::Quantity::parse_with_unit_evidence(&evidence, unit, role.role)
                        .map_err(|reason| {
                            SyntaxCheckError::QuantityLiteral(format!(
                                "invalid constructor coordinate: {reason:?}"
                            ))
                        })?;
                let value = CanonicalStartupValue::Quantity(
                    conduit_core::QuantityConfigurationValue::new(quantity, evidence)
                        .expect("checked resolved constructor evidence"),
                );
                if expected.is_some_and(|ty| crate::authored_quantity::bytes(&value, ty).is_none())
                {
                    return Err(SyntaxCheckError::QuantityLiteral(
                        "quantity constructor family or role differs from expected Type".into(),
                    ));
                }
                return Ok(value);
            }
        }
        if !self.locals.contains_key(&expression.text)
            && !self.parameters.contains_key(&expression.text)
            && !self.runtime_ports.contains(&expression.text)
            && !self
                .runtime_ports
                .iter()
                .any(|name| contains_identifier(&expression.text, name))
        {
            if let Some(value) = crate::physical_declarations::value::parse_value(
                &expression.text,
                expected,
                self.catalog,
            )
            .map_err(|error| match error {
                SyntaxCheckError::QuantityLiteral(message) => {
                    SyntaxCheckError::StructuredExpression(message, Some(expression.span))
                }
                other => other,
            })? {
                return Ok(value);
            }
        }
        if let Some((authored, value)) = self
            .source_values
            .get(&(expression.span.start, expression.span.end))
        {
            if authored != expression || expected.is_some_and(|ty| ty != value.value_type()) {
                return Err(SyntaxCheckError::StructuredExpression(
                    "prepared Source context has incompatible custody or Type".into(),
                    Some(expression.span),
                ));
            }
            return Ok(CanonicalStartupValue::Structured(value.clone()));
        }
        if let crate::ExpressionSyntax::TypedGlyphLiteral(literal) = &expression.syntax {
            return self.resolve_spanned(&literal.authored, expected);
        }
        if let Some(expected) = expected {
            if crate::authored_quantity::expected_role(expected).is_some() {
                return self
                    .resolve_atomic(&expression.text, Some(expected))
                    .map_err(|error| match error {
                        SyntaxCheckError::QuantityLiteral(message) => {
                            SyntaxCheckError::StructuredExpression(message, Some(expression.span))
                        }
                        other => other,
                    });
            }
            let checked = crate::structured_startup::check_structured_expression(
                &expression.syntax,
                expected,
                &mut |atomic, atomic_expected| {
                    self.resolve_spanned(atomic, Some(atomic_expected))
                        .map_err(|error| error.diagnostic(atomic.span))
                },
            )
            .map_err(|diagnostic| {
                SyntaxCheckError::StructuredExpression(diagnostic.message, Some(diagnostic.span))
            })?;
            if !checked.satisfies_concrete_bounds() {
                return Err(SyntaxCheckError::StructuredExpression(
                    "structured value exceeds the finite canonical encoding bound".into(),
                    Some(expression.span),
                ));
            }
            return Ok(CanonicalStartupValue::Structured(checked));
        }
        let negative_integer = matches!(
            &expression.syntax,
            crate::ExpressionSyntax::Unary {
                operator: crate::UnaryOperator::Negate,
                operand,
                ..
            } if matches!(operand.as_ref(), crate::ExpressionSyntax::Atomic(atomic) if looks_integer_magnitude(&atomic.text))
        );
        let negative_quantity = matches!(
            &expression.syntax,
            crate::ExpressionSyntax::Unary {
                operator: crate::UnaryOperator::Negate,
                operand,
                ..
            } if matches!(operand.as_ref(), crate::ExpressionSyntax::Atomic(_))
                && conduit_core::Quantity::parse_plot_literal(&expression.text).is_ok()
        );
        if !matches!(expression.syntax, crate::ExpressionSyntax::Atomic(_))
            && !negative_integer
            && !negative_quantity
        {
            if let Some(runtime) = self
                .runtime_ports
                .iter()
                .find(|name| contains_identifier(&expression.text, name))
            {
                return Err(SyntaxCheckError::RuntimeAsStartup(runtime.clone()));
            }
            return Err(SyntaxCheckError::UnsupportedExpression(
                expression.text.clone(),
            ));
        }
        self.resolve_atomic(&expression.text, None)
    }

    pub(crate) fn resolved_context(
        &self,
    ) -> Option<Vec<(crate::Expression, crate::CanonicalStructuredStartupValue)>> {
        let mut entries = 0usize;
        let mut bytes = 0usize;
        for (name, value) in &self.resolved {
            if let CanonicalStartupValue::Structured(value) = value {
                entries += 1;
                bytes = bytes
                    .saturating_add(self.locals[name].value.text.len())
                    .saturating_add(value.try_concrete()?.canonical_bytes().ok()?.len());
                if entries > 64 || bytes > 1024 * 1024 {
                    return None;
                }
            }
        }
        Some(
            self.resolved
                .iter()
                .filter_map(|(name, value)| match value {
                    CanonicalStartupValue::Structured(value) => {
                        Some((self.locals[name].value.clone(), value.clone()))
                    }
                    _ => None,
                })
                .collect(),
        )
    }

    fn resolve_spanned(
        &mut self,
        source: &crate::SpannedText,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if let Some((literal, value)) = self
            .prepared_glyphs
            .get(&(source.span.start, source.span.end))
        {
            if literal.authored != *source || expected.is_some_and(|ty| ty != value.value_type()) {
                return Err(SyntaxCheckError::StructuredExpression(
                    "prepared glyph has incompatible Source custody or exact Type".into(),
                    Some(source.span),
                ));
            }
            return Ok(CanonicalStartupValue::Structured(value.clone()));
        }
        self.resolve_atomic(&source.text, expected)
    }

    fn resolve_atomic(
        &mut self,
        expression: &str,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if self.locals.contains_key(expression) {
            self.resolve_name(expression, expected)
        } else if self.runtime_ports.contains(expression) {
            Err(SyntaxCheckError::RuntimeAsStartup(expression.to_string()))
        } else if self.parameters.contains_key(expression) {
            if let Some(expected) = expected {
                if crate::authored_quantity::expected_role(expected).is_some()
                    && match expected.shape() {
                        conduit_core::StructuredInfoTypeShape::Leaf(kind) => Some(kind.clone()),
                        _ => expected.profile().ok().map(|p| p.value_kind().clone()),
                    }
                    .as_ref()
                    .is_some_and(|required| {
                        let actual = self.parameters.get(expression).expect("known parameter");
                        required != actual
                            && !(required.as_str() == conduit_core::QUANTITY_INFO_ID
                                && (conduit_core::quantity_info_dimension(actual.as_str())
                                    .is_some()
                                    || conduit_core::primitive_info_kind(actual.as_str())
                                        == Some(conduit_core::PrimitiveInfoKind::Quantity)))
                    })
                {
                    return Err(SyntaxCheckError::QuantityLiteral(format!(
                        "startup parameter '{expression}' has an incompatible exact physical Type"
                    )));
                }
            }
            Ok(CanonicalStartupValue::PlotParameter(expression.to_string()))
        } else if self.pools.contains(expression) {
            Ok(CanonicalStartupValue::PoolReference(
                conduit_core::SharedPoolId::from(expression),
            ))
        } else if let Some(value) =
            crate::physical_declarations::value::parse_value(expression, expected, self.catalog)?
        {
            Ok(value)
        } else if is_atomic_literal(expression) {
            if let Some(role) = expected.and_then(crate::authored_quantity::expected_role) {
                return crate::authored_quantity::parse(expression, role);
            }
            if let Ok(value) = conduit_core::UnitConfigurationValue::parse(expression) {
                return Ok(CanonicalStartupValue::Unit(value));
            }
            match crate::quantity_literal::startup_quantity(expression)? {
                Some(value) => Ok(CanonicalStartupValue::Quantity(value)),
                None => Ok(CanonicalStartupValue::Literal(expression.to_string())),
            }
        } else if let Some(runtime) = self
            .runtime_ports
            .iter()
            .find(|name| contains_identifier(expression, name))
        {
            Err(SyntaxCheckError::RuntimeAsStartup(runtime.clone()))
        } else {
            Err(SyntaxCheckError::UnsupportedExpression(
                expression.to_string(),
            ))
        }
    }
}

fn looks_integer_magnitude(value: &str) -> bool {
    value.starts_with(|character: char| character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_hexdigit() || matches!(character, 'x' | 'b' | 'o' | '_')
        })
}

pub(super) fn is_atomic_literal(expression: &str) -> bool {
    let quoted = (expression.starts_with('"') && expression.ends_with('"'))
        || (expression.starts_with('\'') && expression.ends_with('\''));
    quoted
        || conduit_core::Unit::resolve(expression).is_ok()
        || crate::quantity_literal::compound_token_length(expression) == Some(expression.len())
        || !expression.is_empty()
            && !expression.chars().any(|character| {
                character.is_whitespace()
                    || matches!(
                        character,
                        '(' | ')' | '[' | ']' | '{' | '}' | ',' | '+' | '*' | '/'
                    )
            })
}

fn contains_identifier(expression: &str, name: &str) -> bool {
    expression
        .split(|character: char| !(character.is_alphanumeric() || matches!(character, '_' | '-')))
        .any(|candidate| candidate == name)
}

#[cfg(test)]
mod context_pressure_tests;
