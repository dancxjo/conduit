//! Exact semantic typing for finite, one-input pure expressions.

mod arithmetic;
mod collection;
mod projection;
mod record;
mod structures;
mod value;
use crate::expression_numeric_type::boolean;
use crate::prelude::*;
use crate::{BinaryOperator, ExpressionProjection, ExpressionSyntax, Span, UnaryOperator};
use alloc::collections::{BTreeMap, BTreeSet};
use arithmetic::{binary, unary};
use conduit_core::{
    kind_id, semantic_digest, tuple_info_type, KindId, StructuredFieldType, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoTypeShape,
};
use value::atomic;

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

    pub(crate) fn from_member(value_type: &StructuredInfoType) -> Self {
        structures::member(value_type)
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
        ExpressionSyntax::TypedGlyphLiteral(value) => refuse(
            value.authored.span,
            "typed glyph payload requires its exact ordinary constructor admission",
        ),
        ExpressionSyntax::Input(_) => Ok(context.input.clone()),
        ExpressionSyntax::Atomic(value) => atomic(&value.text, value.span, expected, context),
        ExpressionSyntax::Projection {
            value,
            member,
            span,
        } => {
            if let Some(result) = value::qualified_variant(value, member, *span, context) {
                result
            } else {
                let source = infer(value, None, context, node_types)?;
                projection::check(&source, member, *span, context)
            }
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
            value::variant(tag, payload, *span, expected, context, node_types)
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
