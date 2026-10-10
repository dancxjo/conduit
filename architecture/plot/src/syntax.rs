use crate::prelude::*;
use crate::{CstToken, PlotDiagnostic, Span};

mod front;
mod type_declaration;
pub use front::*;
pub use type_declaration::*;

/// Lossless canonical Conduit source plus its syntax-only AST.
///
/// This layer deliberately does not perform catalog lookup, argument binding,
/// name resolution, or role-specific semantic lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDocument {
    source: String,
    pub tokens: Vec<CstToken>,
    pub uses: Vec<UseDeclaration>,
    pub standard_glyphs: bool,
    pub types: Vec<TypeSyntax>,
    pub type_forms: Vec<TypeFormSyntax>,
    pub plots: Vec<PlotSyntax>,
    pub constructions: Vec<ConstructionSyntax>,
    pub packages: Vec<PackageSyntax>,
    pub diagnostics: Vec<PlotDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SyntaxDefinitions {
    pub types: Vec<TypeSyntax>,
    pub type_forms: Vec<TypeFormSyntax>,
    pub plots: Vec<PlotSyntax>,
    pub constructions: Vec<ConstructionSyntax>,
    pub packages: Vec<PackageSyntax>,
}

impl SyntaxDocument {
    /// Identity of exact authored bytes, independent of catalog resolution.
    pub fn source_document_id(&self) -> conduit_core::SourceDocumentId {
        conduit_core::SourceDocumentId::from(crate::hash_string(&format!(
            "canonical-source:{}",
            self.source
        )))
    }
    pub fn round_trip(&self) -> &str {
        &self.source
    }

    pub fn plots(&self) -> Result<&[PlotSyntax], &PlotDiagnostic> {
        self.diagnostics
            .first()
            .map_or(Ok(self.plots.as_slice()), Err)
    }

    pub fn constructions(&self) -> Result<&[ConstructionSyntax], &PlotDiagnostic> {
        self.diagnostics
            .first()
            .map_or(Ok(self.constructions.as_slice()), Err)
    }

    pub fn packages(&self) -> Result<&[PackageSyntax], &PlotDiagnostic> {
        self.diagnostics
            .first()
            .map_or(Ok(self.packages.as_slice()), Err)
    }

    pub(crate) fn new(
        source: String,
        tokens: Vec<CstToken>,
        uses: Vec<UseDeclaration>,
        standard_glyphs: bool,
        definitions: SyntaxDefinitions,
        diagnostics: Vec<PlotDiagnostic>,
    ) -> Self {
        Self {
            source,
            tokens,
            uses,
            standard_glyphs,
            types: definitions.types,
            type_forms: definitions.type_forms,
            plots: definitions.plots,
            constructions: definitions.constructions,
            packages: definitions.packages,
            diagnostics,
        }
    }
}

/// One finite authored `pack.conduit` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSyntax {
    pub path: SpannedText,
    pub version: SpannedText,
    pub exports: Vec<SpannedText>,
    pub requirements: Vec<PackageRequirementSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequirementSyntax {
    pub path: SpannedText,
    pub version_requirement: SpannedText,
    pub span: Span,
}

/// One explicit source name imported into the document lexical scope.
///
/// The authored path and alias disappear during checking. Checked Gears retain
/// only the exact canonical Kind identity resolved through the supplied source
/// catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseDeclaration {
    pub path: String,
    pub path_span: Span,
    pub alias: SpannedText,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionRole {
    Host,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructionSyntax {
    pub role: ConstructionRole,
    pub name: SpannedText,
    pub declarations: Vec<LocalValue>,
    /// Role-specific declarative policy which is not a runtime expression.
    pub directives: Vec<ConstructionDirectiveSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructionDirectiveSyntax {
    BodyWear { masks: Vec<SpannedText>, span: Span },
    BodyWant { masks: Vec<SpannedText>, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSyntax {
    pub name: SpannedText,
    pub front: PlotFront,
    pub completion: PlotCompletionPolicy,
    /// Lexically private Plots declared in this Plot's back.
    ///
    /// Checking lowers these to ordinary source Plots with unspellable scoped
    /// identities before Fore checking and canonical expansion.
    pub local_plots: Vec<PlotSyntax>,
    pub back: Vec<BackStatement>,
    /// Authored expression-body spelling retained only for source inspection.
    /// The ordinary cord in `back` remains the sole checked meaning.
    pub expression_body: Option<ExpressionBodySyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionBodySyntax {
    pub expression: Expression,
    pub span: Span,
}

/// Authored meaning for what a drained realization means.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlotCompletionPolicy {
    /// The plot remains alive and awaits later admitted work.
    #[default]
    Live,
    /// Draining establishes that this plot's meaning is fulfilled.
    SemanticCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackStatement {
    NamedGear(NamedGear),
    Pool(PoolDeclaration),
    LocalValue(LocalValue),
    Cord(Cord),
    MatchedRoute(MatchedRoute),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRoute {
    pub source: SpannedText,
    pub arms: Vec<MatchedRouteArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRouteArm {
    pub pattern: MatchedRoutePattern,
    pub stages: Vec<CordStage>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchedRoutePattern {
    Variant {
        value_type: SpannedText,
        tag: SpannedText,
        span: Span,
    },
    Guard {
        value_type: SpannedText,
        field: SpannedText,
        expected: Box<Expression>,
        span: Span,
    },
    Otherwise(Span),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolDeclaration {
    pub name: SpannedText,
    pub member_plot: SpannedText,
    pub maximum_members: u16,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedGear {
    pub name: SpannedText,
    pub invocation: Invocation,
    pub retained: Option<Box<RetainedValue>>,
    pub activation: Option<ActivationSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationSyntax {
    Each {
        maximum_items: u16,
    },
    Select {
        maximum_items: u16,
    },
    Fold {
        initial: Box<Expression>,
        maximum_items: u16,
    },
    Scan {
        initial: Box<Expression>,
        maximum_items: u16,
    },
}

impl ActivationSyntax {
    pub fn maximum_items(&self) -> u16 {
        match self {
            Self::Each { maximum_items }
            | Self::Select { maximum_items }
            | Self::Fold { maximum_items, .. }
            | Self::Scan { maximum_items, .. } => *maximum_items,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedDuration {
    Step,
    Play,
    Wake,
    Boot,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedValue {
    pub value_type: SpannedText,
    pub optional: bool,
    pub maximum_bytes: Option<u64>,
    pub initial: Option<Expression>,
    pub duration: RetainedDuration,
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
    /// A punctuation Gear name awaiting lexical resolution during checking.
    Glyph(SpannedText),
    RelationalGlyph {
        operands: Vec<SpannedText>,
        glyph: SpannedText,
        span: Span,
    },
    RelationalGear {
        operands: Vec<SpannedText>,
        invocation: Invocation,
        input_ports: Vec<String>,
        output_port: String,
        span: Span,
    },
    TerminalProjection {
        endpoint: SpannedText,
        terminal: TerminalProjection,
        span: Span,
    },
    Cancellation {
        gear: SpannedText,
        span: Span,
    },
    When(Expression),
    InlineGear(Invocation),
    Literal(Expression),
    PureExpression(Expression),
    StructuredSelector(StructuredSelectorSyntax),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalProjection {
    NormalClose,
    Abnormal,
    Quiescence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredSelectorSyntax {
    Field {
        value_type: SpannedText,
        field: SpannedText,
        span: Span,
    },
    Index {
        value_type: SpannedText,
        index: SpannedText,
        span: Span,
    },
    Variant {
        value_type: SpannedText,
        tag: SpannedText,
        unmatched: SpannedText,
        span: Span,
    },
}

impl StructuredSelectorSyntax {
    pub fn span(&self) -> Span {
        match self {
            Self::Field { span, .. } | Self::Index { span, .. } | Self::Variant { span, .. } => {
                *span
            }
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedText {
    pub text: String,
    pub span: Span,
}
