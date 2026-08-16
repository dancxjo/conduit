use crate::prelude::*;
use crate::{CanonicalStartupValue, SyntaxCheckError};
use alloc::collections::{BTreeMap, BTreeSet};

pub(super) struct Resolver<'a> {
    pub(super) locals: BTreeMap<String, &'a crate::LocalValue>,
    parameters: BTreeSet<String>,
    runtime_ports: BTreeSet<String>,
    pools: BTreeSet<String>,
    resolved: BTreeMap<String, CanonicalStartupValue>,
    visiting: BTreeSet<String>,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(
        locals: BTreeMap<String, &'a crate::LocalValue>,
        parameters: BTreeSet<String>,
        runtime_ports: BTreeSet<String>,
        pools: BTreeSet<String>,
    ) -> Self {
        Self {
            locals,
            parameters,
            runtime_ports,
            pools,
            resolved: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    pub(super) fn resolve_name(
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
        if !self.visiting.insert(name.to_string()) {
            return Err(SyntaxCheckError::DependencyCycle(name.to_string()));
        }
        let expression = self.locals[name].value.clone();
        let value = self.resolve_expression(&expression, expected)?;
        self.visiting.remove(name);
        self.resolved.insert(name.to_string(), value.clone());
        Ok(value)
    }

    pub(super) fn resolve_expression(
        &mut self,
        expression: &crate::Expression,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if let Some(expected) = expected {
            let checked = crate::structured_startup::check_structured_expression(
                &expression.syntax,
                expected,
                &mut |atomic, atomic_expected| {
                    self.resolve_atomic(&atomic.text, Some(atomic_expected))
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
        if !matches!(expression.syntax, crate::ExpressionSyntax::Atomic(_)) {
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
            Ok(CanonicalStartupValue::FormParameter(expression.to_string()))
        } else if self.pools.contains(expression) {
            Ok(CanonicalStartupValue::PoolReference(
                conduit_core::SharedPoolId::from(expression),
            ))
        } else if is_atomic_literal(expression) {
            Ok(CanonicalStartupValue::Literal(expression.to_string()))
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

pub(super) fn is_atomic_literal(expression: &str) -> bool {
    let quoted = (expression.starts_with('"') && expression.ends_with('"'))
        || (expression.starts_with('\'') && expression.ends_with('\''));
    quoted
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
