use conduit_core::{
    CapabilityId, CheckedFormId, ConfigurationEntry, ConfigurationValue, ExpandedFormId,
    FormIdentity, KindContractRevision, KindId, OperationId, PortDescriptor, PortDirection, PortId,
    SourceDocumentId,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod checking;
pub mod parser;

use checking::{checked_form_id, expanded_form_id, exported_contract_revision};
use parser::validate_export_faces;

pub use parser::{parse, parse_document};

pub const MAXIMUM_FORM_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_FORM_TOKENS: usize = 131_072;
pub const MAXIMUM_FORM_NESTING_DEPTH: usize = 16;

/// Exact UTF-8 byte extent plus one-based source locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CstTokenKind {
    Whitespace,
    Comment,
    Lexeme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstToken {
    pub kind: CstTokenKind,
    pub span: Span,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormDiagnostic {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

/// Exact editable source, lossless tokens, and its separately checked meaning.
/// Invalid documents keep their complete source and CST for editor recovery;
/// they never manufacture an executable [`CheckedForm`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormDocument {
    source: String,
    pub tokens: Vec<CstToken>,
    pub checked_form: Option<CheckedForm>,
    pub diagnostics: Vec<FormDiagnostic>,
}

impl FormDocument {
    pub fn round_trip(&self) -> &str {
        &self.source
    }

    pub fn checked(&self) -> Result<&CheckedForm, &FormDiagnostic> {
        self.checked_form.as_ref().ok_or_else(|| {
            self.diagnostics
                .first()
                .expect("an unchecked document always has a diagnostic")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperation {
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<ConfigurationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedConnection {
    pub source_operation_id: OperationId,
    pub source_port_id: PortId,
    pub sink_operation_id: OperationId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedForm {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub name: String,
    pub operations: Vec<CheckedOperation>,
    pub connections: Vec<CheckedConnection>,
    pub exports: Vec<CheckedExport>,
    pub nested_forms: Vec<CheckedNestedForm>,
}

impl CheckedForm {
    pub fn identity(&self) -> FormIdentity {
        FormIdentity {
            source_document_id: self.source_document_id.clone(),
            checked_form_id: self.checked_form_id.clone(),
            expanded_form_id: self.expanded_form_id.clone(),
        }
    }

    /// Recomputes the checked and recursively expanded identities from the
    /// checked structure. This is the drawbridge between mutable hosted data
    /// and planning: callers may not substitute or omit nested expansion rows
    /// while retaining a previously sealed identity.
    pub fn validate_identities(&self) -> Result<(), FormError> {
        for pair in self.nested_forms.windows(2) {
            if pair[0].operation_id >= pair[1].operation_id {
                return Err(FormError::InvalidIdentity(
                    "nested expansion rows are not unique canonical paths".into(),
                ));
            }
        }
        for nested in &self.nested_forms {
            nested.form.validate_identities()?;
            let operation = self
                .operations
                .iter()
                .find(|operation| operation.operation_id == nested.operation_id)
                .ok_or_else(|| {
                    FormError::InvalidIdentity(format!(
                        "nested expansion path '{}' has no checked operation",
                        nested.operation_id.as_str()
                    ))
                })?;
            let boundary = nested
                .form
                .export_boundary_unvalidated(&nested.export_capability_id)?;
            let definition = boundary.kind_definition();
            if operation.kind_id != definition.kind_id
                || operation.kind_contract_revision != definition.kind_contract_revision
                || operation.inputs != definition.inputs
                || operation.outputs != definition.outputs
                || !operation.configuration.is_empty()
            {
                return Err(FormError::InvalidIdentity(format!(
                    "nested expansion path '{}' differs from its selected export",
                    nested.operation_id.as_str()
                )));
            }
        }

        let expected_checked = checked_form_id(
            &self.name,
            &self.operations,
            &self.connections,
            &self.exports,
        );
        if self.checked_form_id != expected_checked {
            return Err(FormError::InvalidIdentity(
                "checked form identity differs from its canonical semantic form".into(),
            ));
        }
        let expected_expanded = expanded_form_id(&expected_checked, &self.nested_forms);
        if self.expanded_form_id != expected_expanded {
            return Err(FormError::InvalidIdentity(
                "expanded form identity omits or substitutes a nested expansion".into(),
            ));
        }
        Ok(())
    }

    /// Derives the only composite boundary contract this form may expose for
    /// `capability_id`. Every field comes from a checked authored export and
    /// its checked endpoint descriptors.
    pub fn export_boundary(
        &self,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, FormError> {
        self.validate_identities()?;
        self.export_boundary_unvalidated(capability_id)
    }

    fn export_boundary_unvalidated(
        &self,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, FormError> {
        let export = self
            .exports
            .iter()
            .find(|export| &export.capability_id == capability_id)
            .ok_or_else(|| {
                FormError::InvalidExport(format!(
                    "checked form has no authored capability '{}'",
                    capability_id.as_str()
                ))
            })?;
        validate_export_faces(export, &self.operations)?;
        let inputs = export
            .input_faces
            .iter()
            .map(|face| face.external_port.clone())
            .collect::<Vec<_>>();
        let outputs = export
            .output_faces
            .iter()
            .map(|face| face.external_port.clone())
            .collect::<Vec<_>>();
        Ok(CheckedCompositeBoundary {
            capability_id: export.capability_id.clone(),
            kind_id: export.kind_id.clone(),
            kind_contract_revision: exported_contract_revision(
                &export.kind_id,
                &export.input_faces,
                &export.output_faces,
            ),
            inputs,
            outputs,
            input_faces: export.input_faces.clone(),
            output_faces: export.output_faces.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNestedForm {
    pub operation_id: OperationId,
    pub export_capability_id: CapabilityId,
    pub form: CheckedForm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedExport {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub input_faces: Vec<CheckedCompositeFace>,
    pub output_faces: Vec<CheckedCompositeFace>,
}

/// Terminal behavior is part of the exported face contract, independently for
/// every face. More policies can be added without weakening the current exact
/// `independent` contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeFaceTerminal {
    Independent,
    /// Reserved invalid value used to prove hosted mutation rejection. The
    /// authored grammar intentionally accepts only `independent` today.
    Coupled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompositeFace {
    pub external_port: PortDescriptor,
    pub internal_operation_id: OperationId,
    pub internal_port_id: PortId,
    pub terminal: CompositeFaceTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompositeBoundary {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub input_faces: Vec<CheckedCompositeFace>,
    pub output_faces: Vec<CheckedCompositeFace>,
}

impl CheckedCompositeBoundary {
    pub fn kind_definition(&self) -> KindDefinition {
        KindDefinition {
            kind_id: self.kind_id.clone(),
            kind_contract_revision: self.kind_contract_revision.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            configuration: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationField {
    pub key: String,
    pub default_value: ConfigurationValue,
    pub validation: ConfigurationRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationRule {
    Any,
    U64Range { minimum: u64, maximum: u64 },
}

impl ConfigurationRule {
    fn accepts(&self, value: &ConfigurationValue) -> bool {
        match (self, value) {
            (Self::Any, _) => true,
            (Self::U64Range { minimum, maximum }, ConfigurationValue::U64(value)) => {
                (*minimum..=*maximum).contains(value)
            }
            (Self::U64Range { .. }, _) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindDefinition {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<ConfigurationField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileCatalog {
    kinds: BTreeMap<KindId, KindDefinition>,
}

impl ProfileCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, definition: KindDefinition) -> Result<(), FormError> {
        if self.kinds.contains_key(&definition.kind_id) {
            return Err(FormError::DuplicateKind(
                definition.kind_id.as_str().to_string(),
            ));
        }
        self.kinds.insert(definition.kind_id.clone(), definition);
        Ok(())
    }

    pub fn get(&self, kind_id: &KindId) -> Option<&KindDefinition> {
        self.kinds.get(kind_id)
    }

    pub fn insert_export(
        &mut self,
        form: &CheckedForm,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, FormError> {
        let boundary = form.export_boundary(capability_id)?;
        self.insert(boundary.kind_definition())?;
        Ok(boundary)
    }

    fn supported_kinds(&self) -> String {
        self.kinds
            .keys()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    SourceLimitExceeded,
    TokenLimitExceeded,
    NestingLimitExceeded,
    InvalidNestedForm(String),
    InvalidHeader,
    IncompleteForm,
    InvalidBlockStart,
    MissingBlockEnd,
    EmptyFormName,
    DuplicateKind(String),
    DuplicateOperation(String),
    UnknownOperation(String),
    UnsupportedKind { kind: String, supported: String },
    InvalidConfiguration(String),
    InvalidConnection(String),
    InvalidExport(String),
    InvalidIdentity(String),
    InvalidStatement(String),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceLimitExceeded => write!(
                f,
                "form source exceeds the {MAXIMUM_FORM_SOURCE_BYTES}-byte limit"
            ),
            Self::TokenLimitExceeded => write!(
                f,
                "form source exceeds the {MAXIMUM_FORM_TOKENS}-token limit"
            ),
            Self::NestingLimitExceeded => write!(
                f,
                "form nesting exceeds the {MAXIMUM_FORM_NESTING_DEPTH}-level limit"
            ),
            Self::InvalidNestedForm(message) => write!(f, "invalid nested form: {message}"),
            Self::InvalidHeader => write!(f, "expected first non-comment line to be 'form 0'"),
            Self::IncompleteForm => write!(f, "incomplete form"),
            Self::InvalidBlockStart => write!(f, "expected form block opener like 'name {{'"),
            Self::MissingBlockEnd => write!(f, "expected closing '}}' at end of form"),
            Self::EmptyFormName => write!(f, "form name must not be empty"),
            Self::DuplicateKind(kind) => write!(f, "duplicate profile kind '{kind}'"),
            Self::DuplicateOperation(name) => write!(f, "duplicate operation '{name}'"),
            Self::UnknownOperation(name) => write!(f, "unknown operation '{name}'"),
            Self::UnsupportedKind { kind, supported } => {
                write!(f, "unsupported kind '{kind}'. supported kinds: {supported}")
            }
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid configuration: {message}")
            }
            Self::InvalidConnection(message) => write!(f, "invalid connection: {message}"),
            Self::InvalidExport(message) => write!(f, "invalid export: {message}"),
            Self::InvalidIdentity(message) => write!(f, "invalid form identity: {message}"),
            Self::InvalidStatement(message) => write!(f, "invalid statement: {message}"),
        }
    }
}

impl std::error::Error for FormError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperationDraft {
    definition: KindDefinition,
    configuration: Vec<ConfigurationEntry>,
}

impl OperationDraft {
    fn new(kind: &str, catalog: &ProfileCatalog) -> Result<Self, FormError> {
        let kind_id = KindId::from(kind);
        let definition =
            catalog
                .get(&kind_id)
                .cloned()
                .ok_or_else(|| FormError::UnsupportedKind {
                    kind: kind.to_string(),
                    supported: catalog.supported_kinds(),
                })?;
        let configuration = definition
            .configuration
            .iter()
            .map(|field| ConfigurationEntry {
                key: field.key.clone(),
                value: field.default_value.clone(),
            })
            .collect();
        Ok(Self {
            definition,
            configuration,
        })
    }
}

#[cfg(test)]
mod tests;
