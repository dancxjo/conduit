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
    NamedCell(NamedCell),
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
pub struct NamedCell {
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
    InlineCell(Invocation),
    Literal(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub operation: SpannedText,
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
    /// Exact expression spelling. Meaning is assigned by #509, not this AST.
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedText {
    pub text: String,
    pub span: Span,
}
