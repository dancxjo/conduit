//! Small target-neutral range proof for checked pure integer expressions.

use crate::{
    BinaryOperator, CheckedExpression, CheckedExpressionType, ExpressionSyntax,
    PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram,
    PortableExpressionProjection,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
};
use conduit_core::{
    primitive_info_kind, PrimitiveInfoKind, StructuredInfoType, StructuredInfoTypeShape,
};

#[derive(Clone, Copy)]
struct Range {
    minimum: i128,
    maximum: i128,
}

#[derive(Clone, Default)]
struct Facts {
    ranges: BTreeMap<String, Range>,
    less_or_equal: BTreeSet<(String, String)>,
}

pub(crate) fn apply(checked: &mut CheckedExpression, invariants: &[PortableExpressionProgram]) {
    let Some(input_kind) = integer_kind(&checked.input_type, &checked.semantic_structures) else {
        // A record input is not itself numeric, but its projected leaves can be.
        let mut facts = Facts::default();
        for invariant in invariants {
            collect_facts(&invariant.root, &mut facts);
        }
        let syntax = checked.syntax.clone();
        analyze(&syntax, checked, &facts);
        return;
    };
    let mut facts = Facts::default();
    facts.ranges.insert(".".into(), full_range(input_kind));
    for invariant in invariants {
        collect_facts(&invariant.root, &mut facts);
    }
    let syntax = checked.syntax.clone();
    analyze(&syntax, checked, &facts);
}

fn collect_facts(node: &PortableExpressionNode, facts: &mut Facts) {
    let PortableExpressionOperation::Binary {
        operator,
        left,
        right,
        ..
    } = &node.operation
    else {
        return;
    };
    if *operator == BinaryOperator::BooleanAnd {
        collect_facts(left, facts);
        collect_facts(right, facts);
        return;
    }
    let left_path = portable_path(left);
    let right_path = portable_path(right);
    if let (Some(left), Some(right)) = (&left_path, &right_path) {
        match operator {
            BinaryOperator::LessOrEqual => {
                facts.less_or_equal.insert((left.clone(), right.clone()));
            }
            BinaryOperator::GreaterOrEqual => {
                facts.less_or_equal.insert((right.clone(), left.clone()));
            }
            BinaryOperator::Less => {
                facts.less_or_equal.insert((left.clone(), right.clone()));
            }
            BinaryOperator::Greater => {
                facts.less_or_equal.insert((right.clone(), left.clone()));
            }
            _ => {}
        }
    }
    match (
        left_path,
        integer_literal(right),
        right_path,
        integer_literal(left),
    ) {
        (Some(path), Some(value), _, _) => constrain(facts, path, *operator, value),
        (_, _, Some(path), Some(value)) => constrain(facts, path, reverse(*operator), value),
        _ => {}
    }
}

fn constrain(facts: &mut Facts, path: String, operator: BinaryOperator, value: i128) {
    let range = facts.ranges.entry(path).or_insert(Range {
        minimum: i128::MIN,
        maximum: i128::MAX,
    });
    match operator {
        BinaryOperator::Less => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::LessOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        _ => {}
    }
}

fn reverse(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn analyze(
    node: &ExpressionSyntax,
    checked: &mut CheckedExpression,
    facts: &Facts,
) -> Option<Range> {
    let target = node_range(node, checked, facts);
    let ExpressionSyntax::Binary {
        operator,
        left,
        right,
        span,
    } = node
    else {
        visit_children(node, checked, facts);
        return target;
    };
    let left_range = analyze(left, checked, facts);
    let right_range = analyze(right, checked, facts);
    if !matches!(
        operator,
        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
    ) {
        return target;
    }
    let Some(kind) = node_kind(node, checked) else {
        return target;
    };
    let allowed = full_range(kind);
    let relational_subtraction =
        *operator == BinaryOperator::Subtract && portable_safety_relation(left, right, facts);
    let computed = match (left_range, right_range, operator) {
        (Some(left), Some(right), BinaryOperator::Add) => Some(Range {
            minimum: left.minimum.checked_add(right.minimum)?,
            maximum: left.maximum.checked_add(right.maximum)?,
        }),
        (Some(left), Some(right), BinaryOperator::Subtract) => Some(Range {
            minimum: left.minimum.checked_sub(right.maximum)?,
            maximum: left.maximum.checked_sub(right.minimum)?,
        }),
        (Some(left), Some(right), BinaryOperator::Multiply) => {
            let values = [
                left.minimum.checked_mul(right.minimum)?,
                left.minimum.checked_mul(right.maximum)?,
                left.maximum.checked_mul(right.minimum)?,
                left.maximum.checked_mul(right.maximum)?,
            ];
            Some(Range {
                minimum: *values.iter().min()?,
                maximum: *values.iter().max()?,
            })
        }
        _ => None,
    };
    if relational_subtraction
        || computed.is_some_and(|range| {
            range.minimum >= allowed.minimum && range.maximum <= allowed.maximum
        })
    {
        checked.proven_arithmetic.insert((span.start, span.end));
        computed.or(Some(allowed))
    } else {
        Some(allowed)
    }
}

fn visit_children(node: &ExpressionSyntax, checked: &mut CheckedExpression, facts: &Facts) {
    match node {
        ExpressionSyntax::Unary { operand, .. } => {
            analyze(operand, checked, facts);
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            analyze(condition, checked, facts);
            let mut when_true_facts = facts.clone();
            collect_syntax_fact(condition, &mut when_true_facts);
            analyze(when_true, checked, &when_true_facts);
            analyze(when_false, checked, facts);
        }
        ExpressionSyntax::Tuple { values, .. } | ExpressionSyntax::Collection { values, .. } => {
            for value in values {
                analyze(value, checked, facts);
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            for field in fields {
                analyze(&field.value, checked, facts);
            }
        }
        ExpressionSyntax::Variant { payload, .. } => {
            analyze(payload, checked, facts);
        }
        ExpressionSyntax::SemanticCall { arguments, .. } => {
            for argument in arguments {
                analyze(argument, checked, facts);
            }
        }
        ExpressionSyntax::Projection { value, .. } => {
            analyze(value, checked, facts);
        }
        ExpressionSyntax::Input(_)
        | ExpressionSyntax::Atomic(_)
        | ExpressionSyntax::Binary { .. } => {}
    }
}

fn collect_syntax_fact(node: &ExpressionSyntax, facts: &mut Facts) {
    let ExpressionSyntax::Binary {
        operator,
        left,
        right,
        ..
    } = node
    else {
        return;
    };
    if *operator == BinaryOperator::BooleanAnd {
        collect_syntax_fact(left, facts);
        collect_syntax_fact(right, facts);
        return;
    }
    match (
        syntax_path(left),
        syntax_integer(right),
        syntax_path(right),
        syntax_integer(left),
    ) {
        (Some(path), Some(value), _, _) => constrain(facts, path, *operator, value),
        (_, _, Some(path), Some(value)) => constrain(facts, path, reverse(*operator), value),
        _ => {}
    }
}

fn syntax_integer(node: &ExpressionSyntax) -> Option<i128> {
    let ExpressionSyntax::Atomic(value) = node else {
        return None;
    };
    parse_integer(&value.text)
}

fn node_range(
    node: &ExpressionSyntax,
    checked: &CheckedExpression,
    facts: &Facts,
) -> Option<Range> {
    if let Some(path) = syntax_path(node) {
        if let Some(range) = facts.ranges.get(&path) {
            return Some(*range);
        }
    }
    if let ExpressionSyntax::Atomic(value) = node {
        return parse_integer(&value.text).map(|value| Range {
            minimum: value,
            maximum: value,
        });
    }
    node_kind(node, checked).map(full_range)
}

fn portable_safety_relation(
    left: &ExpressionSyntax,
    right: &ExpressionSyntax,
    facts: &Facts,
) -> bool {
    let (Some(left), Some(right)) = (syntax_path(left), syntax_path(right)) else {
        return false;
    };
    facts.less_or_equal.contains(&(right, left))
}

fn node_kind(node: &ExpressionSyntax, checked: &CheckedExpression) -> Option<PrimitiveInfoKind> {
    let value_type = &checked
        .node_types
        .iter()
        .find(|candidate| candidate.span == node.span())?
        .value_type;
    integer_kind(value_type, &checked.semantic_structures)
}

fn integer_kind(
    value_type: &CheckedExpressionType,
    structures: &BTreeMap<conduit_core::KindId, StructuredInfoType>,
) -> Option<PrimitiveInfoKind> {
    let kind = value_type.value_kind()?;
    let semantic = structures.get(kind);
    let name = match semantic.map(StructuredInfoType::shape) {
        Some(StructuredInfoTypeShape::Nominal { representation, .. }) => {
            match representation.shape() {
                StructuredInfoTypeShape::Leaf(kind) => kind.as_str(),
                _ => return None,
            }
        }
        _ => kind.as_str(),
    };
    primitive_info_kind(name).filter(|kind| {
        matches!(
            kind,
            PrimitiveInfoKind::U8
                | PrimitiveInfoKind::U16
                | PrimitiveInfoKind::U32
                | PrimitiveInfoKind::U64
                | PrimitiveInfoKind::I8
                | PrimitiveInfoKind::I16
                | PrimitiveInfoKind::I32
                | PrimitiveInfoKind::I64
        )
    })
}

fn full_range(kind: PrimitiveInfoKind) -> Range {
    match kind {
        PrimitiveInfoKind::U8 => Range {
            minimum: 0,
            maximum: u8::MAX.into(),
        },
        PrimitiveInfoKind::U16 => Range {
            minimum: 0,
            maximum: u16::MAX.into(),
        },
        PrimitiveInfoKind::U32 => Range {
            minimum: 0,
            maximum: u32::MAX.into(),
        },
        PrimitiveInfoKind::U64 => Range {
            minimum: 0,
            maximum: i128::from(u64::MAX),
        },
        PrimitiveInfoKind::I8 => Range {
            minimum: i8::MIN.into(),
            maximum: i8::MAX.into(),
        },
        PrimitiveInfoKind::I16 => Range {
            minimum: i16::MIN.into(),
            maximum: i16::MAX.into(),
        },
        PrimitiveInfoKind::I32 => Range {
            minimum: i32::MIN.into(),
            maximum: i32::MAX.into(),
        },
        PrimitiveInfoKind::I64 => Range {
            minimum: i64::MIN.into(),
            maximum: i64::MAX.into(),
        },
        _ => Range {
            minimum: i128::MIN,
            maximum: i128::MAX,
        },
    }
}

fn portable_path(node: &PortableExpressionNode) -> Option<String> {
    match &node.operation {
        PortableExpressionOperation::Input => Some(".".into()),
        PortableExpressionOperation::Projection { value, member } => {
            let mut path = portable_path(value)?;
            if path != "." {
                path.push('.');
            }
            match member {
                PortableExpressionProjection::Field(field) => path.push_str(field),
                PortableExpressionProjection::TupleIndex(index) => {
                    path.push_str(&index.to_string())
                }
            }
            Some(path)
        }
        _ => None,
    }
}

fn syntax_path(node: &ExpressionSyntax) -> Option<String> {
    match node {
        ExpressionSyntax::Input(_) => Some(".".into()),
        ExpressionSyntax::Projection { value, member, .. } => {
            let mut path = syntax_path(value)?;
            if path != "." {
                path.push('.');
            }
            match member {
                crate::ExpressionProjection::Field(field)
                | crate::ExpressionProjection::TupleIndex(field) => path.push_str(&field.text),
            }
            Some(path)
        }
        _ => None,
    }
}

fn integer_literal(node: &PortableExpressionNode) -> Option<i128> {
    let PortableExpressionOperation::Literal(value) = &node.operation else {
        return None;
    };
    parse_integer(value)
}

fn parse_integer(value: &str) -> Option<i128> {
    value.replace('_', "").parse().ok()
}
