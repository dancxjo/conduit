//! Lossless ordinary expression shapes and their source ranges.
use super::SpannedText;
use crate::prelude::*;
use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    /// Exact expression spelling retained independently of parsed shape.
    pub text: String,
    pub syntax: ExpressionSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionSyntax {
    Atomic(SpannedText),
    /// Scoped lexical candidate; only its ordinary owner may admit the payload.
    TypedGlyphLiteral(Box<TypedGlyphLiteralSyntax>),
    Input(Span),
    Projection {
        value: Box<ExpressionSyntax>,
        member: ExpressionProjection,
        span: Span,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<ExpressionSyntax>,
        span: Span,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<ExpressionSyntax>,
        right: Box<ExpressionSyntax>,
        span: Span,
    },
    Conditional {
        condition: Box<ExpressionSyntax>,
        when_true: Box<ExpressionSyntax>,
        when_false: Box<ExpressionSyntax>,
        span: Span,
    },
    Tuple {
        values: Vec<ExpressionSyntax>,
        span: Span,
    },
    Collection {
        values: Vec<ExpressionSyntax>,
        span: Span,
    },
    Record {
        fields: Vec<StructuredExpressionField>,
        span: Span,
    },
    Variant {
        tag: SpannedText,
        payload: Box<ExpressionSyntax>,
        span: Span,
    },
    SemanticCall {
        kind: SpannedText,
        arguments: Vec<ExpressionSyntax>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionProjection {
    Field(SpannedText),
    TupleIndex(SpannedText),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Multiply,
    Divide,
    Remainder,
    Add,
    Subtract,
    ShiftLeft,
    ShiftRight,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
    BitAnd,
    BitXor,
    BitOr,
    BooleanAnd,
    BooleanOr,
}

impl ExpressionSyntax {
    pub fn span(&self) -> Span {
        match self {
            Self::Atomic(value) => value.span,
            Self::TypedGlyphLiteral(value) => value.authored.span,
            Self::Input(span) => *span,
            Self::Projection { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Conditional { span, .. }
            | Self::Tuple { span, .. }
            | Self::Collection { span, .. }
            | Self::Record { span, .. }
            | Self::Variant { span, .. }
            | Self::SemanticCall { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredExpressionField {
    pub name: SpannedText,
    pub value: ExpressionSyntax,
    pub punned: bool,
    pub span: Span,
}

/// Lossless syntax and exact resolved family identity, before payload admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedGlyphLiteralSyntax {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub alias: SpannedText,
    pub delimiter: crate::TypedLiteralDelimiter,
    pub family_identity: [u8; 32],
    pub authored: SpannedText,
    pub raw_payload: SpannedText,
    pub payload: String,
    pub case_insensitive: bool,
    pub anchored_start: bool,
    pub anchored_end: bool,
}
