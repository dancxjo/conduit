//! Substitute compile-time Info into existing portable Type laws.
use super::error;
use crate::prelude::*;
use crate::{
    Expression, ExpressionSyntax as Syntax, SpannedText, StructuredExpressionField,
    SyntaxCheckDiagnostic,
};
use alloc::collections::BTreeMap;

pub(super) fn substitute(
    laws: &[Expression],
    values: &BTreeMap<String, u16>,
) -> Result<Vec<Expression>, SyntaxCheckDiagnostic> {
    let mut nodes = 0;
    laws.iter()
        .map(|law| {
            Ok(Expression {
                text: law.text.clone(),
                span: law.span,
                syntax: rewrite(&law.syntax, values, 0, &mut nodes)?,
            })
        })
        .collect()
}

pub(super) fn uses(laws: &[Expression], parameter: &str) -> bool {
    let mut values = BTreeMap::new();
    values.insert(parameter.into(), 0);
    laws.iter().any(|law| {
        substitute(core::slice::from_ref(law), &values).is_ok_and(|new| new[0].syntax != law.syntax)
    })
}

fn rewrite(
    value: &Syntax,
    values: &BTreeMap<String, u16>,
    depth: usize,
    nodes: &mut usize,
) -> Result<Syntax, SyntaxCheckDiagnostic> {
    *nodes += 1;
    if depth >= 32 || *nodes > 4096 {
        return Err(error(
            value.span(),
            "native Type law substitution exceeds its finite budget".into(),
        ));
    }
    Ok(match value {
        Syntax::Atomic(name) => Syntax::Atomic(SpannedText {
            text: values
                .get(&name.text)
                .map_or_else(|| name.text.clone(), ToString::to_string),
            span: name.span,
        }),
        Syntax::Input(span) => Syntax::Input(*span),
        Syntax::Projection {
            value,
            member,
            span,
        } => Syntax::Projection {
            value: Box::new(rewrite(value, values, depth + 1, nodes)?),
            member: member.clone(),
            span: *span,
        },
        Syntax::Unary {
            operator,
            operand,
            span,
        } => Syntax::Unary {
            operator: *operator,
            operand: Box::new(rewrite(operand, values, depth + 1, nodes)?),
            span: *span,
        },
        Syntax::Binary {
            operator,
            left,
            right,
            span,
        } => Syntax::Binary {
            operator: *operator,
            left: Box::new(rewrite(left, values, depth + 1, nodes)?),
            right: Box::new(rewrite(right, values, depth + 1, nodes)?),
            span: *span,
        },
        Syntax::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => Syntax::Conditional {
            condition: Box::new(rewrite(condition, values, depth + 1, nodes)?),
            when_true: Box::new(rewrite(when_true, values, depth + 1, nodes)?),
            when_false: Box::new(rewrite(when_false, values, depth + 1, nodes)?),
            span: *span,
        },
        Syntax::Tuple {
            values: elements,
            span,
        } => Syntax::Tuple {
            values: rewrite_all(elements, values, depth, nodes)?,
            span: *span,
        },
        Syntax::Collection {
            values: elements,
            span,
        } => Syntax::Collection {
            values: rewrite_all(elements, values, depth, nodes)?,
            span: *span,
        },
        Syntax::Record { fields, span } => Syntax::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(StructuredExpressionField {
                        name: field.name.clone(),
                        value: rewrite(&field.value, values, depth + 1, nodes)?,
                        punned: field.punned,
                        span: field.span,
                    })
                })
                .collect::<Result<_, SyntaxCheckDiagnostic>>()?,
            span: *span,
        },
        Syntax::Variant { tag, payload, span } => Syntax::Variant {
            tag: tag.clone(),
            payload: Box::new(rewrite(payload, values, depth + 1, nodes)?),
            span: *span,
        },
        Syntax::SemanticCall {
            kind,
            arguments,
            span,
        } => Syntax::SemanticCall {
            kind: kind.clone(),
            arguments: rewrite_all(arguments, values, depth, nodes)?,
            span: *span,
        },
    })
}

fn rewrite_all(
    elements: &[Syntax],
    values: &BTreeMap<String, u16>,
    depth: usize,
    nodes: &mut usize,
) -> Result<Vec<Syntax>, SyntaxCheckDiagnostic> {
    elements
        .iter()
        .map(|value| rewrite(value, values, depth + 1, nodes))
        .collect()
}
