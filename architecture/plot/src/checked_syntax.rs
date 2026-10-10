mod type_catalog;
pub use crate::native_type::family::source::NativeTypeSourceOrigin;
use crate::prelude::*;
use crate::{PlotCompletionPolicy, RuntimePort, Span};
use alloc::collections::BTreeMap;
use conduit_core::{CheckedFront, CheckedPlotId, ExpandedPlotId, SourceDocumentId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupParameterSignature {
    pub name: String,
    pub value_type: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindSignature {
    pub kind: String,
    pub startup_parameters: Vec<StartupParameterSignature>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupCatalog {
    kinds: BTreeMap<String, KindSignature>,
    fores: BTreeMap<String, CheckedFront>,
    variadic_fores: BTreeMap<String, crate::HomogeneousVariadicFore>,
    structured_types: BTreeMap<String, conduit_core::StructuredInfoType>,
    structured_type_contracts: BTreeMap<String, Vec<NativeTypeValueContract>>,
    structured_type_invariants: BTreeMap<String, Vec<crate::PortableExpressionProgram>>,
    value_kind_aliases: BTreeMap<String, conduit_core::KindId>,
    pub(crate) native_type_sources: BTreeMap<String, NativeTypeSourceOrigin>,
    pub(crate) native_families: BTreeMap<String, crate::native_type::family::NativeTypeFamily>,
    pub(crate) exact_initial_info: BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
}

impl StartupCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, signature: KindSignature) -> Result<(), String> {
        if self.kinds.contains_key(&signature.kind) {
            return Err(format!(
                "duplicate startup signature for kind '{}'",
                signature.kind
            ));
        }
        let mut names = BTreeMap::new();
        for parameter in &signature.startup_parameters {
            if names.insert(parameter.name.as_str(), ()).is_some() {
                return Err(format!(
                    "duplicate startup parameter '{}' for kind '{}'",
                    parameter.name, signature.kind
                ));
            }
        }
        self.kinds.insert(signature.kind.clone(), signature);
        Ok(())
    }

    pub(crate) fn get(&self, kind: &str) -> Option<&KindSignature> {
        self.kinds.get(kind)
    }

    /// Returns the exact startup Front installed for one semantic Kind.
    ///
    /// Reusable Back installers use this together with the profile contract so
    /// substitution checks the complete Front, including startup parameters.
    pub fn signature(&self, kind: &str) -> Option<&KindSignature> {
        self.kinds.get(kind)
    }

    /// Installs the complete checked Fore for source-time laws which depend on
    /// runtime port shape, such as glyph operand binding. This is not a Back or
    /// planning profile: it carries no implementation or availability truth.
    pub fn insert_fore(&mut self, kind: &str, fore: CheckedFront) -> Result<(), String> {
        let signature = self
            .kinds
            .get(kind)
            .ok_or_else(|| format!("cannot install a Fore for unknown Kind '{kind}'"))?;
        let expected = self
            .canonical_startup_parameters(signature)
            .map_err(|error| format!("invalid startup Fore for Kind '{kind}': {error:?}"))?;
        if fore.startup_parameters() != expected {
            return Err(format!(
                "checked Fore startup parameters differ from Kind '{kind}' signature"
            ));
        }
        if self.fores.contains_key(kind) {
            return Err(format!("duplicate checked Fore for Kind '{kind}'"));
        }
        self.fores.insert(kind.to_string(), fore);
        Ok(())
    }

    pub fn fore(&self, kind: &str) -> Option<&CheckedFront> {
        self.fores.get(kind)
    }

    /// Marks one reviewed Kind Fore as a finite homogeneous input family.
    /// Each use is specialized to ordinary exact ports before checking ends.
    pub fn insert_homogeneous_variadic_fore(
        &mut self,
        kind: &str,
        family: crate::HomogeneousVariadicFore,
    ) -> Result<(), String> {
        let signature = self
            .kinds
            .get(kind)
            .ok_or_else(|| format!("cannot install a variadic Fore for unknown Kind '{kind}'"))?;
        let startup_parameters = self
            .canonical_startup_parameters(signature)
            .map_err(|error| format!("invalid variadic Fore for Kind '{kind}': {error:?}"))?;
        family.specialize(usize::from(family.minimum_inputs()), startup_parameters)?;
        if self.variadic_fores.contains_key(kind) {
            return Err(format!("duplicate variadic Fore for Kind '{kind}'"));
        }
        self.variadic_fores.insert(kind.into(), family);
        Ok(())
    }

    pub(crate) fn fore_for_arity(
        &self,
        kind: &str,
        input_count: usize,
    ) -> Result<Option<CheckedFront>, String> {
        if let Some(family) = self.variadic_fores.get(kind) {
            let signature = self
                .kinds
                .get(kind)
                .expect("variadic Fores retain their Kind signature");
            let startup_parameters = self
                .canonical_startup_parameters(signature)
                .map_err(|error| format!("invalid variadic Fore for Kind '{kind}': {error:?}"))?;
            return family.specialize(input_count, startup_parameters).map(Some);
        }
        Ok(self
            .fores
            .get(kind)
            .filter(|fore| fore.inputs().len() == input_count)
            .cloned())
    }

    /// Resolves authoring spellings into the canonical startup type identities
    /// carried by checked fronts and realization offers.
    pub fn canonical_startup_parameters(
        &self,
        signature: &KindSignature,
    ) -> Result<Vec<conduit_core::FrontStartupParameter>, conduit_core::StructuredInfoRefusal> {
        signature
            .startup_parameters
            .iter()
            .map(|parameter| {
                Ok(conduit_core::FrontStartupParameter {
                    name: parameter.name.clone(),
                    value_type: crate::value_type::checked_value_kind(&parameter.value_type, self)?,
                    has_default: parameter.default.is_some(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalStartupValue {
    Literal(String),
    Quantity(conduit_core::Quantity),
    PlotParameter(String),
    PoolReference(conduit_core::SharedPoolId),
    Structured(crate::CanonicalStructuredStartupValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStartupBinding {
    pub name: String,
    pub value_type: String,
    pub value: CanonicalStartupValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStartupParameter {
    pub name: String,
    pub value_type: String,
    pub default: Option<CanonicalStartupValue>,
    pub optional: bool,
    pub maximum_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CheckedCanonicalGear {
    pub name: Option<String>,
    pub kind: String,
    pub startup_parameters: Vec<conduit_core::FrontStartupParameter>,
    pub startup_bindings: Vec<CheckedStartupBinding>,
    pub retained: Option<Box<CheckedRetainedValue>>,
    pub activation: Option<CheckedActivation>,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedActivation {
    pub mode: crate::ActivationSyntax,
    pub selected_plot: String,
    pub input: conduit_core::PortDescriptor,
    pub accumulator_input: Option<conduit_core::PortDescriptor>,
    pub output: conduit_core::PortDescriptor,
    pub initial_accumulator: Option<CanonicalStartupValue>,
    pub initial_accumulator_bytes: Option<Vec<u8>>,
    pub input_contract: conduit_core::CheckedValueContract,
    pub output_contract: conduit_core::CheckedValueContract,
    pub abnormal_contract: Option<conduit_core::CheckedValueContract>,
    pub accumulator_contract: Option<conduit_core::CheckedValueContract>,
}

/// Canonical checked meaning of one authored `keep` declaration.
///
/// The source type spelling is deliberately gone at this layer. Retained State
/// planning and realization consume the exact structured type and initializer,
/// not an alias or an unchecked expression string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRetainedValue {
    pub value_type: conduit_core::StructuredInfoType,
    pub value_kind: conduit_core::KindId,
    pub optional: bool,
    pub maximum_bytes: Option<u64>,
    pub initial: Option<CanonicalStartupValue>,
    pub duration: crate::RetainedDuration,
}

impl PartialEq for CheckedCanonicalGear {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.kind == other.kind
            && self.startup_parameters == other.startup_parameters
            && self.startup_bindings == other.startup_bindings
            && self.retained == other.retained
            && self.activation == other.activation
    }
}

impl Eq for CheckedCanonicalGear {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCordStage {
    Reference(String),
    RelationalGear {
        operands: Vec<String>,
        gear: CheckedCanonicalGear,
        input_ports: Vec<String>,
        output_port: String,
    },
    TerminalProjection {
        endpoint: String,
        terminal: crate::TerminalProjection,
        source_span: Span,
    },
    Cancellation {
        gear: String,
        source_span: Span,
    },
    When {
        expression: crate::ExpressionSyntax,
        source_span: Span,
    },
    PureExpression {
        expression: crate::ExpressionSyntax,
        source_span: Span,
    },
    InlineGear(CheckedCanonicalGear),
    Literal {
        value: CanonicalStartupValue,
        source_span: Span,
    },
    StructuredSelector {
        selector: conduit_core::StructuredSelector,
        source_span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalCord {
    pub stages: Vec<CheckedCordStage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPoolDeclaration {
    pub name: String,
    pub member_plot: String,
    pub member_front: CheckedFront,
    pub maximum_members: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalPlot {
    pub checked_plot_id: CheckedPlotId,
    pub name: String,
    pub completion: PlotCompletionPolicy,
    pub startup_parameters: Vec<CheckedStartupParameter>,
    pub runtime_ports: Vec<RuntimePort>,
    pub runtime_front: CheckedFront,
    pub shorthand: Option<(String, String)>,
    pub local_values: Vec<(String, CanonicalStartupValue)>,
    pub pools: Vec<CheckedPoolDeclaration>,
    pub gears: Vec<CheckedCanonicalGear>,
    pub cords: Vec<CheckedCanonicalCord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSyntaxDocument {
    pub source_document_id: SourceDocumentId,
    pub native_types: Vec<CheckedNativeType>,
    pub(crate) retained_native_types: Vec<CheckedNativeType>,
    pub type_forms: Vec<CheckedTypeForm>,
    pub plots: Vec<CheckedCanonicalPlot>,
    /// Authored shorthand correlated with the ordinary meaning established by
    /// this exact check. This is source inspection, not another expansion or
    /// an input to planning.
    pub source_sugar_expansions: Vec<SourceSugarExpansion>,
    pub(crate) structured_types: BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    pub(crate) exact_initial_info: BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
}

/// One checked compatibility Form, distinct from semantic Type identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTypeForm {
    pub name: String,
    pub compatibility_id: String,
    pub value_type_name: String,
    pub value_type: conduit_core::KindId,
    pub storage: CheckedTypeFormStorage,
    /// Exact iota order, either derived from Type order or explicitly authored.
    pub mappings: Vec<CheckedTypeFormMapping>,
    pub invalid_refusal: CheckedTypeFormRefusal,
    pub exact_bytes: u16,
    pub maximum_bytes: u16,
    pub maximum_decode_steps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTypeFormStorage {
    U8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTypeFormRefusal {
    InvalidTag,
}

impl CheckedTypeFormRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTag => "invalid_tag",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTypeFormMapping {
    pub variant: String,
    pub discriminant: u8,
}

/// One checked source-owned semantic Type and its exact finite representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNativeType {
    /// Source-local name. Import aliases may change this spelling downstream;
    /// `identity` remains the canonical semantic identity.
    pub name: String,
    pub identity: conduit_core::KindId,
    pub value_type: conduit_core::StructuredInfoType,
    /// Primitive refinement contracts retained at exact representation paths.
    pub value_contracts: Vec<NativeTypeValueContract>,
    /// Pure Boolean laws checked against the complete structured value.
    pub invariants: Vec<crate::PortableExpressionProgram>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTypeValueContract {
    /// Empty for a scalar representation; fields/cases/containers extend it.
    pub representation_path: String,
    pub contract: conduit_core::CheckedValueContract,
}

/// One checked explanation of concise source spelling.
///
/// The ordinary Kind and Fore roles come from the same lexical resolution and
/// Fore specialization used to build [`CheckedCanonicalPlot`]. Consumers must
/// not reinterpret `authored` or use this record as planner input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSugarExpansion {
    pub plot: String,
    pub checked_plot_id: CheckedPlotId,
    pub authored: String,
    pub source_span: Span,
    pub ordinary_kind: String,
    pub input_ports: Vec<String>,
    pub output_ports: Vec<String>,
    pub operand_bindings: Vec<SourceSugarOperandBinding>,
    /// A direct stage replacement when the spelling is losslessly expressible.
    /// Relational applications use explicit port bindings instead.
    pub canonical_replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSugarOperandBinding {
    pub source: String,
    pub input_port: String,
}

impl CheckedSyntaxDocument {
    /// Returns the exact finite structured type behind one checked value Kind.
    ///
    /// Source aliases are deliberately absent here: expression checking and
    /// expansion consume canonical semantic identity, never author spelling.
    pub fn structured_type(
        &self,
        value_kind: &conduit_core::KindId,
    ) -> Option<&conduit_core::StructuredInfoType> {
        self.structured_types.get(value_kind)
    }

    pub(crate) fn structured_types(
        &self,
    ) -> &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType> {
        &self.structured_types
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedGearProvenance {
    pub gear_id: String,
    pub plot_path: Vec<String>,
    pub source_plot: String,
    pub source_gear: String,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCanonicalPlot {
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub expanded_plot_id: ExpandedPlotId,
    pub name: String,
    pub completion: PlotCompletionPolicy,
    pub gears: Vec<crate::CheckedGear>,
    pub connections: Vec<crate::CheckedConnection>,
    pub shared_pools: Vec<ExpandedSharedPool>,
    pub provenance: Vec<ExpandedGearProvenance>,
    pub provenance_digest: String,
    pub realization_backs: Vec<conduit_core::PlotBack>,
    pub activations: Vec<ExpandedActivation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedActivation {
    pub activation_id: String,
    pub owner_gear_id: conduit_core::GearId,
    pub mode: crate::ActivationSyntax,
    pub selected_plot: String,
    pub selected_checked_plot_id: CheckedPlotId,
    pub input: conduit_core::PortDescriptor,
    pub accumulator_input: Option<conduit_core::PortDescriptor>,
    pub output: conduit_core::PortDescriptor,
    pub initial_accumulator: Option<CanonicalStartupValue>,
    pub initial_accumulator_bytes: Option<Vec<u8>>,
    pub input_contract: conduit_core::CheckedValueContract,
    pub output_contract: conduit_core::CheckedValueContract,
    pub abnormal_contract: Option<conduit_core::CheckedValueContract>,
    pub accumulator_contract: Option<conduit_core::CheckedValueContract>,
    pub source_span: Span,
}

/// Canonical graph expansion for authoring an open Back.
///
/// Unlike [`ExpandedCanonicalPlot`] admission through `expand_canonical_plot`, this projection
/// deliberately retains unbound runtime Front Ports. It is not a runnable-root claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedAuthoringPlot {
    pub expanded: ExpandedCanonicalPlot,
    pub front: CheckedFront,
    pub input_bindings: Vec<AuthoringFrontBinding>,
    pub output_bindings: Vec<AuthoringFrontBinding>,
    /// Exact typed abnormal truth which remains unresolved after the Plot's
    /// internal recovery routes. This is inferred checked meaning, not an
    /// authored Fore spelling.
    pub abnormal_export: Option<CheckedPlotAbnormalExport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringFrontBinding {
    pub front_port_id: conduit_core::PortId,
    pub gear_id: conduit_core::GearId,
    pub gear_port_id: conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPlotAbnormalExport {
    pub value_kind: conduit_core::KindId,
    pub gear_id: conduit_core::GearId,
    pub gear_port_id: conduit_core::PortId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedSharedPool {
    pub pool_id: conduit_core::SharedPoolId,
    pub declaration_id: conduit_core::PoolDeclarationId,
    pub member_front: CheckedFront,
    pub maximum_members: u16,
    pub consumers: Vec<conduit_core::GearId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalExpansionDiagnostic {
    pub code: &'static str,
    pub message: String,
}

impl core::fmt::Display for CanonicalExpansionDiagnostic {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl core::error::Error for CanonicalExpansionDiagnostic {}

impl CanonicalExpansionDiagnostic {
    pub(crate) fn new(code: &'static str, message: String) -> Self {
        Self { code, message }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCheckDiagnostic {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyntaxCheckError {
    DuplicateImmutable(String),
    ConflictingArgument(String),
    UnknownParameter(String),
    MissingParameter(String),
    TooManyPositional(String),
    PositionalNamedDuplicate(String),
    DependencyCycle(String),
    RuntimeAsStartup(String),
    UnsupportedKind(String),
    DuplicateGear(String),
    UnsupportedExpression(String),
    QuantityLiteral(String),
    QuantityEligibility(String, Option<Span>),
    InvalidIntegerLiteral(String),
    AmbiguousFrontName(String),
    StructuredExpression(String, Option<Span>),
}

impl SyntaxCheckError {
    pub(crate) fn diagnostic(self, span: Span) -> SyntaxCheckDiagnostic {
        let (code, detail, owned_span) = match self {
            Self::DuplicateImmutable(name) => (
                "CND-FRM-020",
                format!("duplicate immutable binding '{name}'"),
                None,
            ),
            Self::ConflictingArgument(name) => (
                "CND-FRM-021",
                format!("conflicting gear argument for startup parameter '{name}'"),
                None,
            ),
            Self::UnknownParameter(name) => (
                "CND-FRM-022",
                format!("unknown startup parameter '{name}'"),
                None,
            ),
            Self::MissingParameter(name) => (
                "CND-FRM-023",
                format!("missing required startup parameter '{name}'"),
                None,
            ),
            Self::TooManyPositional(gear) => (
                "CND-FRM-024",
                format!("too many positional arguments for '{gear}'"),
                None,
            ),
            Self::PositionalNamedDuplicate(name) => (
                "CND-FRM-025",
                format!("positional and named arguments both bind '{name}'"),
                None,
            ),
            Self::DependencyCycle(name) => (
                "CND-FRM-026",
                format!("startup dependency cycle includes '{name}'"),
                None,
            ),
            Self::RuntimeAsStartup(name) => (
                "CND-FRM-027",
                format!("runtime port '{name}' cannot supply a startup value"),
                None,
            ),
            Self::UnsupportedKind(gear) => (
                "CND-FRM-028",
                format!("no startup signature is available for '{gear}'"),
                None,
            ),
            Self::DuplicateGear(name) => (
                "CND-FRM-029",
                format!("duplicate named gear '{name}'"),
                None,
            ),
            Self::UnsupportedExpression(expression) => (
                "CND-FRM-030",
                format!("unsupported pure startup expression '{expression}'"),
                None,
            ),
            Self::QuantityLiteral(detail) => ("CND-FRM-055", detail, None),
            Self::QuantityEligibility(detail, owned_span) => ("CND-FRM-055", detail, owned_span),
            Self::InvalidIntegerLiteral(detail) => ("CND-FRM-055", detail, None),
            Self::AmbiguousFrontName(name) => (
                "CND-FRM-050",
                format!("front name '{name}' is duplicated or ambiguously shadowed"),
                None,
            ),
            Self::StructuredExpression(detail, owned_span) => ("CND-FRM-051", detail, owned_span),
        };
        SyntaxCheckDiagnostic {
            code,
            span: owned_span.unwrap_or(span),
            message: format!("{detail}; '=' is declarative and there is no later assignment"),
        }
    }
}
