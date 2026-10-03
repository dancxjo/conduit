//! Exact semantic typing for finite, one-input pure expressions.

mod collection;
mod projection;
mod record;
mod structures;
mod variant;

use crate::expression_numeric_type::{
    boolean, is_fixed_integer, is_numeric, is_ordered_numeric, is_signed_numeric,
};
use crate::prelude::*;
use crate::{BinaryOperator, ExpressionProjection, ExpressionSyntax, Span, UnaryOperator};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    kind_id, semantic_digest, tuple_info_type, KindId, StructuredFieldType, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoTypeShape,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckedExpressionType {
    Semantic(KindId),
    Tuple(Vec<CheckedExpressionType>),
    Record(Vec<(String, CheckedExpressionType)>),
    Collection {
        element: Box<CheckedExpressionType>,
        length: u16,
    },
}

impl CheckedExpressionType {
    pub fn semantic(kind: impl AsRef<str>) -> Self {
        Self::Semantic(kind_id(kind.as_ref()))
    }

    fn from_structured(value_type: &StructuredInfoType) -> Self {
        match value_type.shape() {
            StructuredInfoTypeShape::Leaf(kind) => Self::Semantic(kind.clone()),
            StructuredInfoTypeShape::Nominal { .. } => {
                let kind = value_type
                    .profile()
                    .expect("checked nominal types have profiles")
                    .value_kind()
                    .clone();
                Self::Semantic(kind)
            }
            StructuredInfoTypeShape::Collection { element, length } => Self::Collection {
                element: Box::new(structures::member(element)),
                length,
            },
            StructuredInfoTypeShape::Sequence { .. } => {
                let kind = value_type
                    .profile()
                    .expect("checked structured types have profiles")
                    .value_kind()
                    .clone();
                Self::Semantic(kind)
            }
            StructuredInfoTypeShape::Record { fields, .. } => Self::Record(
                fields
                    .iter()
                    .map(|field| {
                        (
                            field.name().to_string(),
                            structures::member(field.value_type()),
                        )
                    })
                    .collect(),
            ),
            StructuredInfoTypeShape::Variant { .. } => {
                let kind = value_type
                    .profile()
                    .expect("checked structured types have profiles")
                    .value_kind()
                    .clone();
                Self::Semantic(kind)
            }
        }
    }

    pub fn value_kind(&self) -> Option<&KindId> {
        match self {
            Self::Semantic(kind) => Some(kind),
            _ => None,
        }
    }

    /// Materializes the exact finite Info type carried by an expression result.
    ///
    /// Anonymous records and tuples receive identities derived solely from their
    /// checked semantic members. Source location, spelling and target do not
    /// participate, so every Host plans the same Port type.
    pub fn structured_info_type(&self) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        self.structured_info_type_with(&BTreeMap::new())
    }

    pub(crate) fn structured_info_type_with(
        &self,
        semantic_structures: &BTreeMap<KindId, StructuredInfoType>,
    ) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        match self {
            Self::Semantic(kind) => semantic_structures
                .get(kind)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| StructuredInfoType::leaf(kind.clone())),
            Self::Collection { element, length } => StructuredInfoType::collection(
                element.structured_info_type_with(semantic_structures)?,
                Some(*length),
            ),
            Self::Tuple(values) => {
                let values = values
                    .iter()
                    .map(|value| value.structured_info_type_with(semantic_structures))
                    .collect::<Result<Vec<_>, _>>()?;
                tuple_info_type(values)
            }
            Self::Record(values) => {
                let fields = values
                    .iter()
                    .map(|(name, value)| {
                        StructuredFieldType::new(
                            name.clone(),
                            value.structured_info_type_with(semantic_structures)?,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                anonymous_record("record", fields)
            }
        }
    }

    pub fn exact_value_kind(&self) -> Result<KindId, StructuredInfoRefusal> {
        match self {
            Self::Semantic(kind) => Ok(kind.clone()),
            _ => Ok(self.structured_info_type()?.profile()?.value_kind().clone()),
        }
    }
}

fn anonymous_record(
    shape: &str,
    mut fields: Vec<StructuredFieldType>,
) -> Result<StructuredInfoType, StructuredInfoRefusal> {
    fields.sort_by(|left, right| left.name().cmp(right.name()));
    let mut identity = Vec::new();
    for field in &fields {
        push_identity_field(&mut identity, field.name());
        let member = field.value_type().profile()?;
        push_identity_field(&mut identity, member.value_kind().as_str());
    }
    let digest = semantic_digest(
        &format!("conduit.conduitese.anonymous-{shape}.v1"),
        &identity,
    );
    let schema = kind_id(&format!(
        "conduitese/anonymous-{shape}-{}@1",
        encode_hex(&digest)
    ));
    StructuredInfoType::record(schema, fields)
}

fn push_identity_field(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&(value.len() as u64).to_le_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedExpression {
    pub syntax: ExpressionSyntax,
    pub input_type: CheckedExpressionType,
    pub value_type: CheckedExpressionType,
    pub node_types: Vec<CheckedExpressionNodeType>,
    /// Arithmetic nodes whose safety follows from declared input-Type laws.
    pub proven_arithmetic: BTreeSet<(usize, usize)>,
    pub(crate) semantic_structures: BTreeMap<KindId, StructuredInfoType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedExpressionNodeType {
    pub span: Span,
    pub value_type: CheckedExpressionType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTypeDiagnostic {
    pub span: Span,
    pub message: String,
}

pub struct ExpressionTypeContext<'a> {
    pub input: &'a CheckedExpressionType,
    pub immutable_values: &'a BTreeMap<String, CheckedExpressionType>,
    pub structured_types: &'a BTreeMap<KindId, StructuredInfoType>,
    /// Semantic literal meanings supplied by the owning catalog.
    ///
    /// In particular, scientific Quantity work can resolve an authored unit
    /// such as `30°C` to its exact dimension without this expression checker
    /// growing a second unit catalog or erasing it to generic Quantity.
    pub literal_types: &'a BTreeMap<String, CheckedExpressionType>,
    /// Semantic numeric contracts admitted by the owning Kind catalog.
    /// Fixed integers and the canonical scalar/count contracts are intrinsic;
    /// domain quantities join only through reviewed semantic registration.
    pub numeric_types: &'a BTreeSet<KindId>,
    pub semantic_kinds: &'a BTreeMap<String, conduit_core::Kind>,
}

pub fn check_expression(
    syntax: &ExpressionSyntax,
    context: &ExpressionTypeContext<'_>,
) -> Result<CheckedExpression, ExpressionTypeDiagnostic> {
    check_expression_as(syntax, None, context)
}

pub(crate) fn check_expression_as(
    syntax: &ExpressionSyntax,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
) -> Result<CheckedExpression, ExpressionTypeDiagnostic> {
    let structures = structures::registry(context.structured_types);
    let context = &ExpressionTypeContext {
        structured_types: &structures,
        input: context.input,
        immutable_values: context.immutable_values,
        literal_types: context.literal_types,
        numeric_types: context.numeric_types,
        semantic_kinds: context.semantic_kinds,
    };
    let mut node_types = Vec::new();
    let value_type = infer(syntax, expected, context, &mut node_types)?;
    node_types.sort_by_key(|node| (node.span.start, node.span.end));
    node_types.dedup_by(|right, left| {
        if right.span.start == left.span.start && right.span.end == left.span.end {
            debug_assert_eq!(right.value_type, left.value_type);
            true
        } else {
            false
        }
    });
    let semantic_structures = context
        .structured_types
        .iter()
        .filter(|(kind, _)| {
            expression_mentions_kind(context.input, kind)
                || expression_mentions_kind(&value_type, kind)
                || node_types
                    .iter()
                    .any(|node| expression_mentions_kind(&node.value_type, kind))
        })
        .map(|(kind, value_type)| (kind.clone(), value_type.clone()))
        .collect();
    Ok(CheckedExpression {
        syntax: syntax.clone(),
        input_type: context.input.clone(),
        value_type,
        node_types,
        proven_arithmetic: BTreeSet::new(),
        semantic_structures,
    })
}

fn expression_mentions_kind(value_type: &CheckedExpressionType, kind: &KindId) -> bool {
    match value_type {
        CheckedExpressionType::Semantic(candidate) => candidate == kind,
        CheckedExpressionType::Tuple(values) => values
            .iter()
            .any(|value| expression_mentions_kind(value, kind)),
        CheckedExpressionType::Record(fields) => fields
            .iter()
            .any(|(_, value)| expression_mentions_kind(value, kind)),
        CheckedExpressionType::Collection { element, .. } => {
            expression_mentions_kind(element, kind)
        }
    }
}

fn infer(
    syntax: &ExpressionSyntax,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let value_type = match syntax {
        ExpressionSyntax::Input(_) => Ok(context.input.clone()),
        ExpressionSyntax::Atomic(value) => atomic(&value.text, value.span, expected, context),
        ExpressionSyntax::Projection {
            value,
            member,
            span,
        } => {
            let source = infer(value, None, context, node_types)?;
            projection::check(&source, member, *span, context)
        }
        ExpressionSyntax::Unary {
            operator,
            operand,
            span,
        } => unary(*operator, operand, *span, expected, context, node_types),
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            span,
        } => binary(*operator, left, right, *span, expected, context, node_types),
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => {
            require(
                infer(condition, Some(&boolean()), context, node_types)?,
                &boolean(),
                condition.span(),
                "conditional condition must be Boolean",
            )?;
            let true_type = infer(when_true, expected, context, node_types)?;
            let false_type = infer(when_false, Some(&true_type), context, node_types)?;
            require(
                false_type,
                &true_type,
                *span,
                "conditional branches must have one exact type",
            )?;
            Ok(true_type)
        }
        ExpressionSyntax::Tuple { values, span } => {
            if values.is_empty() {
                return refuse(*span, "tuple must contain at least one value");
            }
            let expected_values = match expected {
                Some(CheckedExpressionType::Tuple(expected_values))
                    if expected_values.len() == values.len() =>
                {
                    Some(expected_values.as_slice())
                }
                _ => None,
            };
            let checked = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    infer(
                        value,
                        expected_values.and_then(|types| types.get(index)),
                        context,
                        node_types,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CheckedExpressionType::Tuple(checked))
        }
        ExpressionSyntax::Record { fields, span } => {
            record::record(fields, *span, expected, context, node_types)
        }
        ExpressionSyntax::Collection { values, span } => {
            collection::check(values, *span, expected, context, node_types)
        }
        ExpressionSyntax::Variant { tag, payload, span } => {
            variant::check(tag, payload, *span, expected, context, node_types)
        }
        ExpressionSyntax::SemanticCall {
            kind,
            arguments,
            span,
        } => crate::expression_semantic_call::check(
            kind,
            arguments,
            *span,
            context,
            |argument, expected| infer(argument, expected, context, node_types),
        ),
    }?;
    node_types.push(CheckedExpressionNodeType {
        span: syntax.span(),
        value_type: value_type.clone(),
    });
    Ok(value_type)
}

fn atomic(
    text: &str,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if let Some(value_type) = context.immutable_values.get(text) {
        return Ok(value_type.clone());
    }
    if let Some(value_type) = context.literal_types.get(text) {
        return expected_or_exact(value_type.clone(), expected, span);
    }
    if text == "unit" {
        return expected_or_exact(
            CheckedExpressionType::semantic(conduit_core::UNIT_INFO_ID),
            expected,
            span,
        );
    }
    if matches!(text, "true" | "false") {
        return expected_or_exact(boolean(), expected, span);
    }
    if crate::text_value::parse_quoted_text(text).is_some() {
        return expected_or_exact(
            CheckedExpressionType::semantic("value/text"),
            expected,
            span,
        );
    }
    if let Ok(quantity) = conduit_core::Quantity::parse_plot_literal(text) {
        return expected_or_exact(
            CheckedExpressionType::semantic(quantity.dimension().info_id()),
            expected,
            span,
        );
    }
    let Some(expected) = expected else {
        return refuse(
            span,
            "numeric literal needs an exact semantic type from its expression context",
        );
    };
    let Some(kind) = expected.value_kind() else {
        return refuse(span, "literal cannot inhabit this structural type");
    };
    let represented_kind = context
        .structured_types
        .get(kind)
        .and_then(|value_type| match value_type.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                match representation.shape() {
                    StructuredInfoTypeShape::Leaf(kind) => Some(kind.as_str()),
                    _ => None,
                }
            }
            _ => None,
        })
        .unwrap_or(kind.as_str());
    if crate::integer_literal::canonicalize(text, represented_kind)
        .map_err(|message| diagnostic(span, &message))?
        .is_some()
        || matches!(represented_kind, "value/count" | "value/scalar")
    {
        return Ok(expected.clone());
    }
    refuse(span, "literal is incompatible with its exact expected type")
}

fn unary(
    operator: UnaryOperator,
    operand: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    match operator {
        UnaryOperator::Not => {
            let actual = infer(operand, Some(&boolean()), context, node_types)?;
            require(actual, &boolean(), span, "! requires Boolean")?;
            Ok(boolean())
        }
        UnaryOperator::Negate => {
            let actual = infer(operand, expected, context, node_types)?;
            if is_signed_numeric(&actual, context) {
                Ok(actual)
            } else {
                refuse(span, "unary - requires an exact signed numeric type")
            }
        }
    }
}

fn binary(
    operator: BinaryOperator,
    left: &ExpressionSyntax,
    right: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let expected = boolean();
        require(
            infer(left, Some(&expected), context, node_types)?,
            &expected,
            left.span(),
            "Boolean operator requires Boolean operands",
        )?;
        require(
            infer(right, Some(&expected), context, node_types)?,
            &expected,
            right.span(),
            "Boolean operator requires Boolean operands",
        )?;
        return Ok(expected);
    }
    let checkpoint = node_types.len();
    let left_type = match infer(left, expected, context, node_types) {
        Ok(value_type) => value_type,
        Err(left_error) => {
            node_types.truncate(checkpoint);
            match infer(right, expected, context, node_types) {
                Ok(right_type) => {
                    infer(left, Some(&right_type), context, node_types).map_err(|_| left_error)?
                }
                Err(_) => return Err(left_error),
            }
        }
    };
    let right_type = infer(right, Some(&left_type), context, node_types)?;
    require(
        right_type,
        &left_type,
        span,
        "binary operands must have one exact type",
    )?;
    match operator {
        BinaryOperator::Equal | BinaryOperator::NotEqual => Ok(boolean()),
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
            if is_ordered_numeric(&left_type, context) =>
        {
            Ok(boolean())
        }
        BinaryOperator::BitAnd | BinaryOperator::BitXor | BinaryOperator::BitOr
            if is_fixed_integer(&left_type, context) =>
        {
            Ok(left_type)
        }
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            if is_fixed_integer(&left_type, context) =>
        {
            Ok(left_type)
        }
        BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Add
        | BinaryOperator::Subtract
            if is_numeric(&left_type, context) =>
        {
            Ok(left_type
                .value_kind()
                .and_then(|kind| context.structured_types.get(kind))
                .and_then(|value_type| match value_type.shape() {
                    StructuredInfoTypeShape::Nominal { representation, .. } => {
                        match representation.shape() {
                            StructuredInfoTypeShape::Leaf(kind) => {
                                Some(CheckedExpressionType::Semantic(kind.clone()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .unwrap_or(left_type))
        }
        _ => refuse(span, "operator is not defined for this exact type"),
    }
}

fn expected_or_exact(
    exact: CheckedExpressionType,
    expected: Option<&CheckedExpressionType>,
    span: Span,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if let Some(expected) = expected {
        require(exact, expected, span, "literal has the wrong exact type")?;
        Ok(expected.clone())
    } else {
        Ok(exact)
    }
}

fn require(
    actual: CheckedExpressionType,
    expected: &CheckedExpressionType,
    span: Span,
    message: &str,
) -> Result<(), ExpressionTypeDiagnostic> {
    if &actual == expected {
        Ok(())
    } else {
        refuse(span, message)
    }
}

fn diagnostic(span: Span, message: &str) -> ExpressionTypeDiagnostic {
    ExpressionTypeDiagnostic {
        span,
        message: message.to_string(),
    }
}

fn refuse<T>(span: Span, message: &str) -> Result<T, ExpressionTypeDiagnostic> {
    Err(diagnostic(span, message))
}
