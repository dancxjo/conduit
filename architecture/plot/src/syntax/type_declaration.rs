//! Authored nominal Type and portable Form syntax.
use super::{Expression, SpannedText, ValueRefinement};
use crate::prelude::*;
use crate::Span;

mod integer;
pub use integer::*;

/// A native family parameter's declaration determines its argument kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTypeParameterSyntax {
    pub name: SpannedText,
    /// None declares a Type parameter; Some declares a compile-time Info value.
    pub value_type: Option<Box<TypeExpressionSyntax>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTypeArgumentSyntax {
    Type(Box<TypeExpressionSyntax>),
    Value(Box<NativeIntegerExpressionSyntax>),
}

/// One named finite Form for carrying a nominal semantic Type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFormSyntax {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub storage: TypeFormStorageSyntax,
    /// First iota discriminant. Defaults to zero.
    pub first_discriminant: u8,
    /// Empty means semantic variant order. Otherwise this is iota order.
    pub mappings: Vec<TypeFormMappingSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeFormStorageSyntax {
    U8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFormMappingSyntax {
    pub variant: SpannedText,
    pub span: Span,
}

/// One authored nominal semantic Type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSyntax {
    pub name: SpannedText,
    /// Checked portable Type parameters. These are compile-time semantic
    /// placeholders and never survive in a runtime value.
    pub parameters: Vec<NativeTypeParameterSyntax>,
    /// Checker-owned canonical generic declaration and argument provenance.
    /// Parsed declarations always leave this empty.
    pub(crate) generic_context: Option<String>,
    pub definition: TypeDefinitionSyntax,
    /// Pure Boolean laws every value of this Type must satisfy.
    pub invariants: Vec<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDefinitionSyntax {
    Scalar(TypeExpressionSyntax),
    Record(Vec<TypeFieldSyntax>),
    Variant(Vec<TypeVariantCaseSyntax>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFieldSyntax {
    pub name: SpannedText,
    pub value_type: TypeExpressionSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeVariantCaseSyntax {
    pub tag: SpannedText,
    pub payload: TypeVariantPayloadSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeVariantPayloadSyntax {
    Unit,
    Type(TypeExpressionSyntax),
    Record(Vec<TypeFieldSyntax>),
}

/// Finite structural representation used inside one nominal Type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpressionSyntax {
    Reference {
        value_type: SpannedText,
        /// Exact semantic arguments for an authored generic Type.
        arguments: Vec<NativeTypeArgumentSyntax>,
        maximum_bytes: Option<u64>,
        refinements: Vec<ValueRefinement>,
        span: Span,
    },
    Optional {
        value: Box<TypeExpressionSyntax>,
        span: Span,
    },
    DataReference {
        value: Box<TypeExpressionSyntax>,
        span: Span,
    },
    Collection {
        element: Box<TypeExpressionSyntax>,
        length: Box<NativeIntegerExpressionSyntax>,
        span: Span,
    },
    Sequence {
        element: Box<TypeExpressionSyntax>,
        minimum_items: Box<NativeIntegerExpressionSyntax>,
        maximum_items: Box<NativeIntegerExpressionSyntax>,
        span: Span,
    },
}
