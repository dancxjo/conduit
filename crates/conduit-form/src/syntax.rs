use crate::prelude::*;
use crate::{CstToken, FormDiagnostic, Span};

/// Lossless canonical Form source plus its syntax-only AST.
///
/// This layer deliberately does not perform catalog lookup, argument binding,
/// name resolution, or semantic canonicalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDocument {
    source: String,
    pub tokens: Vec<CstToken>,
    pub forms: Vec<FormSyntax>,
    pub diagnostics: Vec<FormDiagnostic>,
}

impl SyntaxDocument {
    pub fn round_trip(&self) -> &str {
        &self.source
    }

    pub fn forms(&self) -> Result<&[FormSyntax], &FormDiagnostic> {
        self.diagnostics
            .first()
            .map_or(Ok(self.forms.as_slice()), Err)
    }

    pub(crate) fn new(
        source: String,
        tokens: Vec<CstToken>,
        forms: Vec<FormSyntax>,
        diagnostics: Vec<FormDiagnostic>,
    ) -> Self {
        Self {
            source,
            tokens,
            forms,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSyntax {
    pub name: SpannedText,
    pub face: FormFace,
    pub back: Vec<BackStatement>,
    pub span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormFace {
    pub startup_parameters: Vec<StartupParameter>,
    pub runtime_ports: Vec<RuntimePort>,
    pub shorthand: Option<ShorthandPair>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupParameter {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub default: Option<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePortTemporal {
    Value,
    Flow { closes: bool },
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePort {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub direction: RuntimePortDirection,
    pub temporal: RuntimePortTemporal,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShorthandPair {
    pub input_port: SpannedText,
    pub output_port: SpannedText,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackStatement {
    NamedGear(NamedGear),
    Pool(PoolDeclaration),
    LocalValue(LocalValue),
    Cord(Cord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolDeclaration {
    pub name: SpannedText,
    pub member_form: SpannedText,
    pub maximum_members: u16,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedGear {
    pub name: SpannedText,
    pub invocation: Invocation,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalValue {
    pub name: SpannedText,
    pub value: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cord {
    pub stages: Vec<CordStage>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CordStage {
    Reference(SpannedText),
    InlineGear(Invocation),
    Literal(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub kind: SpannedText,
    pub arguments: Vec<Argument>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argument {
    Positional(Expression),
    Named {
        name: SpannedText,
        value: Expression,
        span: Span,
    },
}

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
}

impl ExpressionSyntax {
    pub fn span(&self) -> Span {
        match self {
            Self::Atomic(value) => value.span,
            Self::Collection { span, .. }
            | Self::Record { span, .. }
            | Self::Variant { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredExpressionField {
    pub name: SpannedText,
    pub value: ExpressionSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedText {
    pub text: String,
    pub span: Span,
}
