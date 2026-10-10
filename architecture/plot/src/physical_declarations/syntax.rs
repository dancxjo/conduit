use crate::prelude::*;
use crate::{ExpressionSyntax, Span, SpannedText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionDeclarationSyntax {
    pub name: SpannedText,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixDeclarationSyntax {
    pub group: SpannedText,
    pub symbol: SpannedText,
    pub exponent: ExpressionSyntax,
    /// Alternative spelling points at another declared prefix in this group.
    pub alias: Option<SpannedText>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionPowerSyntax {
    pub dimension: SpannedText,
    pub power: ExpressionSyntax,
    pub span: Span,
}

/// Part of an ordinary named Type declaration, not a separate Type namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantityDefinitionSyntax {
    Linear {
        dimensions: Vec<DimensionPowerSyntax>,
        span: Span,
    },
    Point {
        difference_type: SpannedText,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrefixPolicySyntax {
    pub si: bool,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitReferenceSyntax {
    Origin(Span),
    NamedOrigin { name: SpannedText, span: Span },
    Unit(SpannedText),
}

/// Scale and offset remain exact ordinary expression syntax until admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTransformSyntax {
    pub reference: UnitReferenceSyntax,
    pub scale: ExpressionSyntax,
    pub offset: Option<ExpressionSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferenceUnitSyntax {
    pub quantity: SpannedText,
    pub transform: UnitTransformSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDeclarationSyntax {
    pub symbol: SpannedText,
    pub quantity_type: SpannedText,
    pub transform: UnitTransformSyntax,
    pub difference: Option<DifferenceUnitSyntax>,
    pub prefixes: PrefixPolicySyntax,
    /// Prefix scale is raised to this exact positive integer power. Defaults to
    /// one; area and volume declarations explicitly select two or three.
    pub prefix_power: Option<ExpressionSyntax>,
    pub span: Span,
}
