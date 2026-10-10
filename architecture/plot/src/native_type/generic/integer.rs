//! Finite U16 evaluation before runtime Type construction.
use super::error;
use crate::prelude::*;
use crate::{
    NativeIntegerExpressionSyntax as Integer, NativeIntegerOperator as Operator,
    SyntaxCheckDiagnostic,
};
use alloc::collections::BTreeMap;

pub(super) fn normalize(
    expression: &Integer,
    bindings: &BTreeMap<String, u16>,
) -> Result<Integer, SyntaxCheckDiagnostic> {
    let value = evaluate(expression, bindings, 0, &mut 0)?;
    Ok(Integer::Literal {
        value,
        span: expression.span(),
    })
}

pub(super) fn extent(
    expression: &Integer,
    bindings: &BTreeMap<String, u16>,
    allow_zero: bool,
) -> Result<Integer, SyntaxCheckDiagnostic> {
    let normalized = normalize(expression, bindings)?;
    let value = normalized
        .literal_value()
        .expect("normalization returns a literal");
    if (!allow_zero && value == 0)
        || usize::from(value) > conduit_core::MAXIMUM_STRUCTURED_COLLECTION_ITEMS
    {
        return Err(error(
            expression.span(),
            "native collection extent exceeds its finite profile or is zero where forbidden".into(),
        ));
    }
    Ok(normalized)
}

fn evaluate(
    expression: &Integer,
    bindings: &BTreeMap<String, u16>,
    depth: usize,
    nodes: &mut usize,
) -> Result<u16, SyntaxCheckDiagnostic> {
    *nodes += 1;
    if depth >= 64 || *nodes > 64 {
        return Err(error(
            expression.span(),
            "native integer expression exceeds its finite evaluation budget".into(),
        ));
    }
    match expression {
        Integer::Literal { value, .. } => Ok(*value),
        Integer::Parameter(name) => bindings.get(&name.text).copied().ok_or_else(|| {
            error(
                name.span,
                alloc::format!("native integer parameter '{}' is not bound", name.text),
            )
        }),
        Integer::Group { value, .. } => evaluate(value, bindings, depth + 1, nodes),
        Integer::Binary {
            operator,
            left,
            right,
            operator_span,
            ..
        } => {
            let left = evaluate(left, bindings, depth + 1, nodes)?;
            let right = evaluate(right, bindings, depth + 1, nodes)?;
            match operator {
                Operator::Add => left.checked_add(right),
                Operator::Multiply => left.checked_mul(right),
            }
            .ok_or_else(|| {
                error(
                    *operator_span,
                    "native integer arithmetic overflows U16".into(),
                )
            })
        }
    }
}

pub(super) fn canonical(expression: &Integer) -> String {
    match expression {
        Integer::Literal { value, .. } => value.to_string(),
        Integer::Parameter(name) => alloc::format!("parameter:{}", name.text),
        Integer::Group { value, .. } => canonical(value),
        Integer::Binary {
            operator,
            left,
            right,
            ..
        } => alloc::format!("({}{operator:?}{})", canonical(left), canonical(right)),
    }
}

#[cfg(test)]
mod tests;
