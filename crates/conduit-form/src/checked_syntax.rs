use crate::{RuntimePort, Span};
use conduit_core::{CheckedFormId, SourceDocumentId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupParameterSignature {
    pub name: String,
    pub value_type: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSignature {
    pub operation: String,
    pub startup_parameters: Vec<StartupParameterSignature>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupCatalog {
    operations: BTreeMap<String, OperationSignature>,
}

impl StartupCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, signature: OperationSignature) -> Result<(), String> {
        if self.operations.contains_key(&signature.operation) {
            return Err(format!(
                "duplicate startup signature for operation '{}'",
                signature.operation
            ));
        }
        let mut names = BTreeMap::new();
        for parameter in &signature.startup_parameters {
            if names.insert(parameter.name.as_str(), ()).is_some() {
                return Err(format!(
                    "duplicate startup parameter '{}' for operation '{}'",
                    parameter.name, signature.operation
                ));
            }
        }
        self.operations
            .insert(signature.operation.clone(), signature);
        Ok(())
    }

    pub(crate) fn get(&self, operation: &str) -> Option<&OperationSignature> {
        self.operations.get(operation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalStartupValue {
    Literal(String),
    FormParameter(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalCell {
    pub name: Option<String>,
    pub operation: String,
    pub startup_bindings: Vec<CheckedStartupBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalForm {
    pub checked_form_id: CheckedFormId,
    pub name: String,
    pub startup_parameters: Vec<CheckedStartupParameter>,
    pub runtime_ports: Vec<RuntimePort>,
    pub shorthand: Option<(String, String)>,
    pub local_values: Vec<(String, CanonicalStartupValue)>,
    pub cells: Vec<CheckedCanonicalCell>,
    pub cords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSyntaxDocument {
    pub source_document_id: SourceDocumentId,
    pub forms: Vec<CheckedCanonicalForm>,
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
    UnsupportedOperation(String),
    DuplicateCell(String),
    UnsupportedExpression(String),
}

impl SyntaxCheckError {
    pub(crate) fn diagnostic(self, span: Span) -> SyntaxCheckDiagnostic {
        let (code, detail) = match self {
            Self::DuplicateImmutable(name) => (
                "CND-FRM-020",
                format!("duplicate immutable binding '{name}'"),
            ),
            Self::ConflictingArgument(name) => (
                "CND-FRM-021",
                format!("conflicting cell argument for startup parameter '{name}'"),
            ),
            Self::UnknownParameter(name) => {
                ("CND-FRM-022", format!("unknown startup parameter '{name}'"))
            }
            Self::MissingParameter(name) => (
                "CND-FRM-023",
                format!("missing required startup parameter '{name}'"),
            ),
            Self::TooManyPositional(operation) => (
                "CND-FRM-024",
                format!("too many positional arguments for '{operation}'"),
            ),
            Self::PositionalNamedDuplicate(name) => (
                "CND-FRM-025",
                format!("positional and named arguments both bind '{name}'"),
            ),
            Self::DependencyCycle(name) => (
                "CND-FRM-026",
                format!("startup dependency cycle includes '{name}'"),
            ),
            Self::RuntimeAsStartup(name) => (
                "CND-FRM-027",
                format!("runtime port '{name}' cannot supply a startup value"),
            ),
            Self::UnsupportedOperation(operation) => (
                "CND-FRM-028",
                format!("no startup signature is available for '{operation}'"),
            ),
            Self::DuplicateCell(name) => ("CND-FRM-029", format!("duplicate named cell '{name}'")),
            Self::UnsupportedExpression(expression) => (
                "CND-FRM-030",
                format!("unsupported pure startup expression '{expression}'"),
            ),
        };
        SyntaxCheckDiagnostic {
            code,
            span,
            message: format!("{detail}; '=' is declarative and there is no later assignment"),
        }
    }
}
