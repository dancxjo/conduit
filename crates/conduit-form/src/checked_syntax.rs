use crate::prelude::*;
use crate::{RuntimePort, Span};
use alloc::collections::BTreeMap;
use conduit_core::{CheckedFace, CheckedFormId, ExpandedFormId, SourceDocumentId};

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
    structured_types: BTreeMap<String, conduit_core::StructuredInfoType>,
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

    pub fn insert_structured_type(
        &mut self,
        name: impl Into<String>,
        value_type: conduit_core::StructuredInfoType,
    ) -> Result<(), String> {
        let name = name.into();
        if name.is_empty() {
            return Err("structured startup type name must not be empty".into());
        }
        if self.structured_types.contains_key(&name) {
            return Err(format!("duplicate structured startup type '{name}'"));
        }
        self.structured_types.insert(name, value_type);
        Ok(())
    }

    pub(crate) fn structured_type(&self, name: &str) -> Option<&conduit_core::StructuredInfoType> {
        self.structured_types.get(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalStartupValue {
    Literal(String),
    FormParameter(String),
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
}

#[derive(Debug, Clone)]
pub struct CheckedCanonicalGear {
    pub name: Option<String>,
    pub kind: String,
    pub startup_parameters: Vec<StartupParameterSignature>,
    pub startup_bindings: Vec<CheckedStartupBinding>,
    pub source_span: Span,
}

impl PartialEq for CheckedCanonicalGear {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.kind == other.kind
            && self.startup_parameters == other.startup_parameters
            && self.startup_bindings == other.startup_bindings
    }
}

impl Eq for CheckedCanonicalGear {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCordStage {
    Reference(String),
    InlineGear(CheckedCanonicalGear),
    Literal {
        value: CanonicalStartupValue,
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
    pub member_form: String,
    pub member_face: CheckedFace,
    pub maximum_members: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalForm {
    pub checked_form_id: CheckedFormId,
    pub name: String,
    pub startup_parameters: Vec<CheckedStartupParameter>,
    pub runtime_ports: Vec<RuntimePort>,
    pub shorthand: Option<(String, String)>,
    pub local_values: Vec<(String, CanonicalStartupValue)>,
    pub pools: Vec<CheckedPoolDeclaration>,
    pub gears: Vec<CheckedCanonicalGear>,
    pub cords: Vec<CheckedCanonicalCord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSyntaxDocument {
    pub source_document_id: SourceDocumentId,
    pub forms: Vec<CheckedCanonicalForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedGearProvenance {
    pub gear_id: String,
    pub form_path: Vec<String>,
    pub source_form: String,
    pub source_gear: String,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCanonicalForm {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub name: String,
    pub gears: Vec<crate::CheckedGear>,
    pub connections: Vec<crate::CheckedConnection>,
    pub shared_pools: Vec<ExpandedSharedPool>,
    pub provenance: Vec<ExpandedGearProvenance>,
    pub provenance_digest: String,
    pub realization_backs: Vec<conduit_core::RealizationBack>,
}

/// Canonical graph expansion for authoring an open Back.
///
/// Unlike [`ExpandedCanonicalForm`] admission through `expand_canonical_form`, this projection
/// deliberately retains unbound runtime Face Ports. It is not a runnable-root claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedAuthoringForm {
    pub expanded: ExpandedCanonicalForm,
    pub face: CheckedFace,
    pub input_bindings: Vec<AuthoringFaceBinding>,
    pub output_bindings: Vec<AuthoringFaceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringFaceBinding {
    pub face_port_id: conduit_core::PortId,
    pub gear_id: conduit_core::GearId,
    pub gear_port_id: conduit_core::PortId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedSharedPool {
    pub pool_id: conduit_core::SharedPoolId,
    pub declaration_id: conduit_core::PoolDeclarationId,
    pub member_face: CheckedFace,
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
    AmbiguousFaceName(String),
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
            Self::AmbiguousFaceName(name) => (
                "CND-FRM-050",
                format!("face name '{name}' is duplicated or ambiguously shadowed"),
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
