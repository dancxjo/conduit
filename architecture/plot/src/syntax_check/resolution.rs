use crate::prelude::*;
use crate::{CanonicalStartupValue, SyntaxCheckError};
use alloc::collections::{BTreeMap, BTreeSet};

pub(crate) struct Resolver<'a> {
    context_budget: Option<(usize, usize)>,
    pub(crate) locals: BTreeMap<String, &'a crate::LocalValue>,
    parameters: BTreeSet<String>,
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
        parameters: BTreeSet<String>,
        runtime_ports: BTreeSet<String>,
        pools: BTreeSet<String>,
        prepared_glyphs: &'a BTreeMap<
            (usize, usize),
            (
                crate::TypedGlyphLiteralSyntax,
                crate::CanonicalStructuredStartupValue,
            ),
        >,
        source_values: &'a BTreeMap<
            (usize, usize),
            (crate::Expression, crate::CanonicalStructuredStartupValue),
        >,
    ) -> Self {
        Self {
            context_budget: None,
            locals,
            parameters,
            runtime_ports,
            pools,
            resolved: BTreeMap::new(),
            visiting: BTreeSet::new(),
            prepared_glyphs,
            source_values,
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
                && matches!(conduit_core::Quantity::parse_plot_literal(&expression.text),
                    Ok(_) | Err(conduit_core::QuantityLiteralRefusal::RepresentationIneligible { .. }
                        | conduit_core::QuantityLiteralRefusal::NonCanonicalUnit { .. }
                        | conduit_core::QuantityLiteralRefusal::AmbiguousUnit))
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
            .map_err(|error| match error {
                SyntaxCheckError::QuantityEligibility(detail, None) => {
                    SyntaxCheckError::QuantityEligibility(detail, Some(expression.span))
                }
                error => error,
            })
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
        } else if self.parameters.contains(expression) {
            Ok(CanonicalStartupValue::PlotParameter(expression.to_string()))
        } else if self.pools.contains(expression) {
            Ok(CanonicalStartupValue::PoolReference(
                conduit_core::SharedPoolId::from(expression),
            ))
        } else if is_atomic_literal(expression) {
            if expected.is_some_and(crate::quantity_literal::is_exact_profile) {
                // Preserve raw authored spelling until the selected leaf codec
                // validates semantic suffix and finite representation together.
                return Ok(CanonicalStartupValue::Literal(expression.to_string()));
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
