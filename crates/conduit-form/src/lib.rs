use conduit_core::{
    CapabilityId, CheckedFormId, ConfigurationEntry, ConfigurationValue, ExpandedFormId,
    FormIdentity, KindContractRevision, KindId, OperationId, PortDescriptor, PortId,
    SourceDocumentId,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
        let source = self
            .operations
            .iter()
            .find(|operation| operation.operation_id == export.source_operation_id)
            .ok_or_else(|| {
                FormError::InvalidExport("export source operation is not checked".into())
            })?;
        let output = source
            .outputs
            .iter()
            .find(|port| port.port_id == export.source_port_id)
            .cloned()
            .ok_or_else(|| FormError::InvalidExport("export source port is not checked".into()))?;
        let sink = self
            .operations
            .iter()
            .find(|operation| operation.operation_id == export.sink_operation_id)
            .ok_or_else(|| {
                FormError::InvalidExport("export sink operation is not checked".into())
            })?;
        let input = sink
            .inputs
            .iter()
            .find(|port| port.port_id == export.sink_port_id)
            .ok_or_else(|| FormError::InvalidExport("export sink port is not checked".into()))?;
        if output.value_kind != export.value_kind || input.value_kind != export.value_kind {
            return Err(FormError::InvalidExport(
                "export value kind differs from its checked endpoints".into(),
            ));
        }
        Ok(CheckedCompositeBoundary {
            capability_id: export.capability_id.clone(),
            kind_id: export.kind_id.clone(),
            kind_contract_revision: exported_contract_revision(
                &export.kind_id,
                &[],
                std::slice::from_ref(&output),
            ),
            inputs: Vec::new(),
            outputs: vec![output],
            source_operation_id: export.source_operation_id.clone(),
            source_port_id: export.source_port_id.clone(),
            sink_operation_id: export.sink_operation_id.clone(),
            sink_port_id: export.sink_port_id.clone(),
            value_kind: export.value_kind.clone(),
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
    pub source_operation_id: OperationId,
    pub source_port_id: PortId,
    pub sink_operation_id: OperationId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompositeBoundary {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub source_operation_id: OperationId,
    pub source_port_id: PortId,
    pub sink_operation_id: OperationId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
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

pub fn parse_document(source: &str, catalog: &ProfileCatalog) -> FormDocument {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        let error = FormError::SourceLimitExceeded;
        return FormDocument {
            source: String::new(),
            tokens: Vec::new(),
            checked_form: None,
            diagnostics: vec![diagnostic(error, whole_source_span(source))],
        };
    }

    let tokens = match tokenize_losslessly(source) {
        Ok(tokens) => tokens,
        Err(span) => {
            let error = FormError::TokenLimitExceeded;
            return FormDocument {
                source: source.to_string(),
                tokens: Vec::new(),
                checked_form: None,
                diagnostics: vec![diagnostic(error, span)],
            };
        }
    };
    match parse_checked_with_span(source, catalog) {
        Ok(checked_form) => FormDocument {
            source: source.to_string(),
            tokens,
            checked_form: Some(checked_form),
            diagnostics: Vec::new(),
        },
        Err((error, span)) => FormDocument {
            source: source.to_string(),
            tokens,
            checked_form: None,
            diagnostics: vec![diagnostic(error, span)],
        },
    }
}

pub fn parse(source: &str, catalog: &ProfileCatalog) -> Result<CheckedForm, FormError> {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return Err(FormError::SourceLimitExceeded);
    }
    parse_checked_with_span(source, catalog).map_err(|(error, _)| error)
}

#[derive(Debug, Clone, Copy)]
struct LocatedLine<'a> {
    text: &'a str,
    span: Span,
}

fn parse_checked_with_span(
    source: &str,
    catalog: &ProfileCatalog,
) -> Result<CheckedForm, (FormError, Span)> {
    let lines = significant_lines(source);
    let eof = eof_span(source);
    let first_span = lines.first().map_or(eof, |line| line.span);
    if lines.first().map_or("", |line| line.text) != "form 0" {
        return Err((FormError::InvalidHeader, first_span));
    }
    if lines.len() < 2 {
        return Err((FormError::IncompleteForm, eof));
    }
    let (form, next) = parse_form_block(source, &lines, 1, catalog, 0, Some(source))?;
    if next != lines.len() {
        return Err((
            FormError::InvalidStatement(lines[next].text.to_string()),
            lines[next].span,
        ));
    }
    Ok(form)
}

fn parse_form_block(
    source: &str,
    lines: &[LocatedLine<'_>],
    start: usize,
    catalog: &ProfileCatalog,
    depth: usize,
    identity_source: Option<&str>,
) -> Result<(CheckedForm, usize), (FormError, Span)> {
    let header = lines
        .get(start)
        .copied()
        .ok_or_else(|| (FormError::IncompleteForm, eof_span(source)))?;
    if depth > MAXIMUM_FORM_NESTING_DEPTH {
        return Err((FormError::NestingLimitExceeded, header.span));
    }
    if !header.text.ends_with('{') {
        return Err((FormError::InvalidBlockStart, header.span));
    }
    let declaration = header.text.trim_end_matches('{').trim();
    let name = if identity_source.is_some() {
        declaration
    } else {
        declaration
            .split_once(':')
            .map_or(declaration, |(name, _)| name)
    }
    .trim()
    .to_string();
    if name.is_empty() {
        return Err((FormError::EmptyFormName, header.span));
    }

    let mut operations = BTreeMap::<String, OperationDraft>::new();
    let mut connections = Vec::<CheckedConnection>::new();
    let mut exports = Vec::<CheckedExport>::new();
    let mut nested_forms = Vec::<CheckedNestedForm>::new();
    let mut index = start + 1;
    while index < lines.len() {
        let located = lines[index];
        let line = located.text;
        if line == "}" {
            nested_forms.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
            let checked_operations = operations
                .iter()
                .map(|(operation_name, draft)| CheckedOperation {
                    operation_id: OperationId::from(operation_name.as_str()),
                    kind_id: draft.definition.kind_id.clone(),
                    kind_contract_revision: draft.definition.kind_contract_revision.clone(),
                    inputs: draft.definition.inputs.clone(),
                    outputs: draft.definition.outputs.clone(),
                    configuration: draft.configuration.clone(),
                })
                .collect::<Vec<_>>();
            let checked_form_id =
                checked_form_id(&name, &checked_operations, &connections, &exports);
            let checked_source =
                identity_source.unwrap_or_else(|| &source[header.span.start..located.span.end]);
            let source_document_id =
                SourceDocumentId::from(hash_string(&format!("source-document:{checked_source}")));
            let expanded_form_id = expanded_form_id(&checked_form_id, &nested_forms);
            return Ok((
                CheckedForm {
                    source_document_id,
                    checked_form_id,
                    expanded_form_id,
                    name,
                    operations: checked_operations,
                    connections,
                    exports,
                    nested_forms,
                },
                index + 1,
            ));
        }
        if line.ends_with('{') {
            let nested_declaration = line.trim_end_matches('{').trim();
            let (operation_name, capability_name) =
                nested_declaration.split_once(':').ok_or_else(|| {
                    (
                        FormError::InvalidNestedForm("expected 'operation: capability {'".into()),
                        located.span,
                    )
                })?;
            let operation_name = operation_name.trim();
            let capability_name = capability_name.trim();
            if operation_name.is_empty() || capability_name.is_empty() {
                return Err((FormError::InvalidBlockStart, located.span));
            }
            if operations.contains_key(operation_name) {
                return Err((
                    FormError::DuplicateOperation(operation_name.to_string()),
                    located.span,
                ));
            }
            let (nested_form, next) =
                parse_form_block(source, lines, index, catalog, depth + 1, None)?;
            let export_capability_id = CapabilityId::from(capability_name);
            let boundary = nested_form
                .export_boundary(&export_capability_id)
                .map_err(|error| (error, located.span))?;
            operations.insert(
                operation_name.to_string(),
                OperationDraft {
                    definition: boundary.kind_definition(),
                    configuration: Vec::new(),
                },
            );
            nested_forms.push(CheckedNestedForm {
                operation_id: OperationId::from(operation_name),
                export_capability_id,
                form: nested_form,
            });
            index = next;
            continue;
        }
        if let Some(export) = line.strip_prefix("export ") {
            let export = parse_export(export, &operations, &connections)
                .map_err(|error| (error, located.span))?;
            if exports
                .iter()
                .any(|checked| checked.capability_id == export.capability_id)
            {
                return Err((
                    FormError::InvalidExport(format!(
                        "duplicate capability '{}'",
                        export.capability_id.as_str()
                    )),
                    located.span,
                ));
            }
            exports.push(export);
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once(':') {
            let operation_id = left.trim().to_string();
            if operations.contains_key(&operation_id) {
                return Err((FormError::DuplicateOperation(operation_id), located.span));
            }
            operations.insert(
                operation_id,
                OperationDraft::new(right.trim(), catalog)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once('=') {
            let (operation_id, key) = left.trim().split_once('.').ok_or_else(|| {
                (
                    FormError::InvalidConfiguration(line.to_string()),
                    located.span,
                )
            })?;
            let operation = operations.get_mut(operation_id.trim()).ok_or_else(|| {
                (
                    FormError::UnknownOperation(operation_id.trim().to_string()),
                    located.span,
                )
            })?;
            let entry = operation
                .configuration
                .iter_mut()
                .find(|entry| entry.key == key.trim())
                .ok_or_else(|| {
                    (
                        FormError::InvalidConfiguration(format!(
                            "unsupported key '{}' for '{}'",
                            key.trim(),
                            operation.definition.kind_id.as_str()
                        )),
                        located.span,
                    )
                })?;
            let value = parse_configuration_value(right.trim(), &entry.value)
                .map_err(|error| (error, located.span))?;
            let field = operation
                .definition
                .configuration
                .iter()
                .find(|field| field.key == key.trim())
                .expect("configuration entry came from its catalog field");
            if !field.validation.accepts(&value) {
                return Err((
                    FormError::InvalidConfiguration(format!(
                        "value for '{}.{}' violates the profile catalog rule",
                        operation_id.trim(),
                        key.trim()
                    )),
                    located.span,
                ));
            }
            entry.value = value;
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once("->") {
            connections.push(
                parse_connection(left.trim(), right.trim(), &operations)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once('>') {
            connections.push(
                parse_shorthand_connection(left.trim(), right.trim(), &operations)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        return Err((FormError::InvalidStatement(line.to_string()), located.span));
    }
    Err((FormError::MissingBlockEnd, eof_span(source)))
}

fn significant_lines(source: &str) -> Vec<LocatedLine<'_>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for (line_index, raw) in source.split_inclusive('\n').enumerate() {
        let content = raw.strip_suffix('\n').unwrap_or(raw);
        let trimmed_start = content.trim_start();
        let leading = content.len() - trimmed_start.len();
        let leading_columns = content[..leading].chars().count();
        let text = trimmed_start.trim_end();
        if !text.is_empty() && !text.starts_with('#') {
            let start = offset + leading;
            let end = start + text.len();
            lines.push(LocatedLine {
                text,
                span: Span {
                    start,
                    end,
                    line: line_index + 1,
                    column: leading_columns + 1,
                    end_line: line_index + 1,
                    end_column: leading_columns + text.chars().count() + 1,
                },
            });
        }
        offset += raw.len();
    }
    lines
}

fn tokenize_losslessly(source: &str) -> Result<Vec<CstToken>, Span> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    let mut column = 1;

    while offset < source.len() {
        let start = offset;
        let start_line = line;
        let start_column = column;
        let first = source[offset..]
            .chars()
            .next()
            .expect("offset is inside source");
        let kind;

        if first.is_whitespace() {
            kind = CstTokenKind::Whitespace;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if !next.is_whitespace() {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else if first == '#' {
            kind = CstTokenKind::Comment;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if next == '\n' {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else {
            kind = CstTokenKind::Lexeme;
            let quote = matches!(first, '\'' | '"').then_some(first);
            let mut escaped = false;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if quote.is_none() && (next.is_whitespace() || next == '#') {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
                if let Some(quote) = quote {
                    if next == quote && offset > start + next.len_utf8() && !escaped {
                        break;
                    }
                    escaped = next == '\\' && !escaped;
                }
            }
        }

        let span = Span {
            start,
            end: offset,
            line: start_line,
            column: start_column,
            end_line: line,
            end_column: column,
        };
        if tokens.len() == MAXIMUM_FORM_TOKENS {
            return Err(span);
        }
        tokens.push(CstToken {
            kind,
            span,
            text: source[start..offset].to_string(),
        });
    }
    Ok(tokens)
}

fn advance(character: char, offset: &mut usize, line: &mut usize, column: &mut usize) {
    *offset += character.len_utf8();
    if character == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

fn whole_source_span(source: &str) -> Span {
    let end = eof_span(source);
    Span {
        start: 0,
        end: source.len(),
        line: 1,
        column: 1,
        end_line: end.line,
        end_column: end.column,
    }
}

fn eof_span(source: &str) -> Span {
    let mut line = 1;
    let mut column = 1;
    for character in source.chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Span {
        start: source.len(),
        end: source.len(),
        line,
        column,
        end_line: line,
        end_column: column,
    }
}

fn diagnostic(error: FormError, span: Span) -> FormDiagnostic {
    let code = match error {
        FormError::InvalidHeader => "CND-FRM-001",
        FormError::IncompleteForm => "CND-FRM-002",
        FormError::InvalidBlockStart => "CND-FRM-003",
        FormError::MissingBlockEnd => "CND-FRM-004",
        FormError::EmptyFormName => "CND-FRM-005",
        FormError::DuplicateKind(_) => "CND-FRM-006",
        FormError::DuplicateOperation(_) => "CND-FRM-007",
        FormError::UnknownOperation(_) => "CND-FRM-008",
        FormError::UnsupportedKind { .. } => "CND-FRM-009",
        FormError::InvalidConfiguration(_) => "CND-FRM-010",
        FormError::InvalidConnection(_) => "CND-FRM-011",
        FormError::InvalidExport(_) => "CND-FRM-012",
        FormError::InvalidStatement(_) => "CND-FRM-013",
        FormError::SourceLimitExceeded => "CND-FRM-014",
        FormError::TokenLimitExceeded => "CND-FRM-015",
        FormError::NestingLimitExceeded => "CND-FRM-016",
        FormError::InvalidNestedForm(_) => "CND-FRM-017",
        FormError::InvalidIdentity(_) => "CND-FRM-018",
    };
    FormDiagnostic {
        code,
        span,
        message: error.to_string(),
    }
}

fn parse_export(
    source: &str,
    operations: &BTreeMap<String, OperationDraft>,
    connections: &[CheckedConnection],
) -> Result<CheckedExport, FormError> {
    let (declaration, boundary) = source
        .split_once('=')
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    let (capability_id, kind_id) = declaration
        .trim()
        .split_once(':')
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    let (left, right) = boundary
        .trim()
        .split_once("->")
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    let checked = parse_connection(left.trim(), right.trim(), operations)?;
    if !connections.contains(&checked) {
        return Err(FormError::InvalidExport(
            "export boundary must name an already-authored connection".to_string(),
        ));
    }
    if capability_id.trim().is_empty() || kind_id.trim().is_empty() {
        return Err(FormError::InvalidExport(source.to_string()));
    }
    Ok(CheckedExport {
        capability_id: CapabilityId::from(capability_id.trim()),
        kind_id: KindId::from(kind_id.trim()),
        source_operation_id: checked.source_operation_id,
        source_port_id: checked.source_port_id,
        sink_operation_id: checked.sink_operation_id,
        sink_port_id: checked.sink_port_id,
        value_kind: checked.value_kind,
    })
}

fn parse_configuration_value(
    source: &str,
    expected: &ConfigurationValue,
) -> Result<ConfigurationValue, FormError> {
    match expected {
        ConfigurationValue::Bool(_) => match source {
            "true" => Ok(ConfigurationValue::Bool(true)),
            "false" => Ok(ConfigurationValue::Bool(false)),
            _ => Err(FormError::InvalidConfiguration(format!(
                "invalid boolean '{source}'"
            ))),
        },
        ConfigurationValue::U64(_) => source
            .parse()
            .map(ConfigurationValue::U64)
            .map_err(|_| FormError::InvalidConfiguration(format!("invalid integer '{source}'"))),
    }
}

fn parse_connection(
    left: &str,
    right: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let (source_operation, source_port) = parse_endpoint(left)?;
    let (sink_operation, sink_port) = parse_endpoint(right)?;
    connection_from_ports(
        source_operation,
        source_port,
        sink_operation,
        sink_port,
        operations,
    )
}

fn parse_shorthand_connection(
    source_operation: &str,
    sink_operation: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    if source.definition.outputs.len() != 1 || sink.definition.inputs.len() != 1 {
        return Err(FormError::InvalidConnection(format!(
            "shorthand requires exactly one output and one input for '{source_operation} > {sink_operation}'"
        )));
    }
    connection_from_ports(
        source_operation,
        source.definition.outputs[0].port_id.as_str(),
        sink_operation,
        sink.definition.inputs[0].port_id.as_str(),
        operations,
    )
}

fn connection_from_ports(
    source_operation: &str,
    source_port: &str,
    sink_operation: &str,
    sink_port: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    let source_descriptor = source
        .definition
        .outputs
        .iter()
        .find(|port| port.port_id.as_str() == source_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{source_operation}' has no output port '{source_port}'"
            ))
        })?;
    let sink_descriptor = sink
        .definition
        .inputs
        .iter()
        .find(|port| port.port_id.as_str() == sink_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{sink_operation}' has no input port '{sink_port}'"
            ))
        })?;
    if source_descriptor.value_kind != sink_descriptor.value_kind {
        return Err(FormError::InvalidConnection(format!(
            "value kind '{}' cannot connect to '{}'",
            source_descriptor.value_kind.as_str(),
            sink_descriptor.value_kind.as_str()
        )));
    }
    Ok(CheckedConnection {
        source_operation_id: OperationId::from(source_operation),
        source_port_id: source_descriptor.port_id.clone(),
        sink_operation_id: OperationId::from(sink_operation),
        sink_port_id: sink_descriptor.port_id.clone(),
        value_kind: source_descriptor.value_kind.clone(),
    })
}

fn operation<'a>(
    operations: &'a BTreeMap<String, OperationDraft>,
    operation_id: &str,
) -> Result<&'a OperationDraft, FormError> {
    operations
        .get(operation_id)
        .ok_or_else(|| FormError::UnknownOperation(operation_id.to_string()))
}

fn parse_endpoint(endpoint: &str) -> Result<(&str, &str), FormError> {
    endpoint.split_once('.').ok_or_else(|| {
        FormError::InvalidConnection(format!("expected explicit port in '{endpoint}'"))
    })
}

fn canonical_form_text(
    name: &str,
    operations: &[CheckedOperation],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> String {
    let mut text = format!("form:{name}\n");
    for operation in operations {
        text.push_str(&format!(
            "op:{}:{}:{}|",
            operation.operation_id.as_str(),
            operation.kind_id.as_str(),
            operation.kind_contract_revision.as_str()
        ));
        for port in operation.inputs.iter().chain(&operation.outputs) {
            let direction = match port.direction {
                conduit_core::PortDirection::Input => "input",
                conduit_core::PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "port:{}:{}:{}|",
                port.port_id.as_str(),
                port.value_kind.as_str(),
                direction
            ));
        }
        for entry in &operation.configuration {
            text.push_str(&format!(
                "cfg:{}={}|",
                entry.key,
                render_value(&entry.value)
            ));
        }
    }
    for connection in connections {
        text.push_str(&format!(
            "conn:{}:{}->{}:{}|",
            connection.source_operation_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_operation_id.as_str(),
            connection.sink_port_id.as_str()
        ));
    }
    for export in exports {
        text.push_str(&format!(
            "export:{}:{}:{}:{}->{}:{}:{}|",
            export.capability_id.as_str(),
            export.kind_id.as_str(),
            export.source_operation_id.as_str(),
            export.source_port_id.as_str(),
            export.sink_operation_id.as_str(),
            export.sink_port_id.as_str(),
            export.value_kind.as_str()
        ));
    }
    text
}

fn checked_form_id(
    name: &str,
    operations: &[CheckedOperation],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> CheckedFormId {
    CheckedFormId::from(hash_string(&canonical_form_text(
        name,
        operations,
        connections,
        exports,
    )))
}

fn expanded_form_id(
    checked_form_id: &CheckedFormId,
    nested_forms: &[CheckedNestedForm],
) -> ExpandedFormId {
    let mut canonical = format!("expanded-form:{}", checked_form_id.as_str());
    for nested in nested_forms {
        canonical.push_str("|nested:");
        push_identity_field(&mut canonical, nested.operation_id.as_str());
        push_identity_field(&mut canonical, nested.export_capability_id.as_str());
        push_identity_field(&mut canonical, nested.form.expanded_form_id.as_str());
    }
    ExpandedFormId::from(hash_string(&canonical))
}

fn exported_contract_revision(
    kind_id: &KindId,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
) -> KindContractRevision {
    let mut canonical = String::from("checked-export-contract:");
    push_identity_field(&mut canonical, kind_id.as_str());
    for (direction, ports) in [("input", inputs), ("output", outputs)] {
        for port in ports {
            push_identity_field(&mut canonical, direction);
            push_identity_field(&mut canonical, port.port_id.as_str());
            push_identity_field(&mut canonical, port.value_kind.as_str());
        }
    }
    KindContractRevision::from(format!("checked-export:{}", hash_string(&canonical)))
}

fn push_identity_field(canonical: &mut String, value: &str) {
    canonical.push_str(&value.len().to_string());
    canonical.push(':');
    canonical.push_str(value);
    canonical.push('|');
}

fn render_value(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
    }
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse, parse_document, ConfigurationField, ConfigurationRule, FormError, KindDefinition,
        ProfileCatalog, MAXIMUM_FORM_NESTING_DEPTH, MAXIMUM_FORM_SOURCE_BYTES, MAXIMUM_FORM_TOKENS,
    };
    use conduit_core::{
        kind_id, port_id, CapabilityId, ConfigurationValue, KindContractRevision, PortDescriptor,
        PortDirection,
    };

    fn catalog() -> ProfileCatalog {
        catalog_with_source_contract("test/source@1", "out", "test/value")
    }

    fn catalog_with_source_contract(
        source_revision: &str,
        source_port: &str,
        value_kind: &str,
    ) -> ProfileCatalog {
        let value_kind = kind_id(value_kind);
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: kind_id("test/source"),
                kind_contract_revision: KindContractRevision::from(source_revision),
                inputs: Vec::new(),
                outputs: vec![PortDescriptor {
                    port_id: port_id(source_port),
                    value_kind: value_kind.clone(),
                    direction: PortDirection::Output,
                }],
                configuration: vec![ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(1),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: 4,
                    },
                }],
            })
            .expect("source kind installs");
        catalog
            .insert(KindDefinition {
                kind_id: kind_id("test/sink"),
                kind_contract_revision: KindContractRevision::from("test/sink@1"),
                inputs: vec![PortDescriptor {
                    port_id: port_id("in"),
                    value_kind,
                    direction: PortDirection::Input,
                }],
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .expect("sink kind installs");
        catalog
    }

    #[test]
    fn checked_form_identity_binds_contract_revision_and_ports() {
        let source =
            "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let baseline = parse(source, &catalog()).expect("baseline parses");
        let revised = parse(
            source,
            &catalog_with_source_contract("test/source@2", "out", "test/value"),
        )
        .expect("revised contract parses");
        let renamed_port = parse(
            source,
            &catalog_with_source_contract("test/source@1", "renamed", "test/value"),
        )
        .expect("renamed port parses");
        let retyped_port = parse(
            source,
            &catalog_with_source_contract("test/source@1", "out", "test/value-v2"),
        )
        .expect("retyped port parses");

        assert_ne!(baseline.checked_form_id, revised.checked_form_id);
        assert_ne!(baseline.checked_form_id, renamed_port.checked_form_id);
        assert_ne!(baseline.checked_form_id, retyped_port.checked_form_id);
        assert_eq!(baseline.source_document_id, revised.source_document_id);
        assert_ne!(baseline.expanded_form_id, revised.expanded_form_id);
        assert_ne!(baseline.expanded_form_id, renamed_port.expanded_form_id);
        assert_ne!(baseline.expanded_form_id, retyped_port.expanded_form_id);
    }

    #[test]
    fn source_checked_and_expanded_form_identities_stay_distinct() {
        let baseline_source =
            "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let spelling_only_source = "# author note\nform 0\nidentity {\n\n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let semantic_change_source = "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source.count = 2\n source > sink\n}\n";

        let baseline = parse(baseline_source, &catalog()).expect("baseline parses");
        let spelling_only =
            parse(spelling_only_source, &catalog()).expect("spelling-only edit parses");
        let semantic_change =
            parse(semantic_change_source, &catalog()).expect("semantic edit parses");

        assert_ne!(
            baseline.source_document_id,
            spelling_only.source_document_id
        );
        assert_eq!(baseline.checked_form_id, spelling_only.checked_form_id);
        assert_eq!(baseline.expanded_form_id, spelling_only.expanded_form_id);

        assert_ne!(
            baseline.source_document_id,
            semantic_change.source_document_id
        );
        assert_ne!(baseline.checked_form_id, semantic_change.checked_form_id);
        assert_ne!(baseline.expanded_form_id, semantic_change.expanded_form_id);
    }

    #[test]
    fn lossless_document_round_trips_utf8_comments_and_layout() {
        let source = "# café\r\nform 0\n\n  δemo {  \n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let document = parse_document(source, &catalog());
        let checked = document.checked().expect("document checks");
        let compatibility = parse(source, &catalog()).expect("compatibility parser checks");

        assert_eq!(document.round_trip(), source);
        assert_eq!(
            document
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<String>(),
            source
        );
        for token in &document.tokens {
            assert_eq!(
                source.get(token.span.start..token.span.end),
                Some(token.text.as_str())
            );
        }
        assert_eq!(checked, &compatibility);
        assert!(document.diagnostics.is_empty());
    }

    #[test]
    fn lossless_document_retains_later_source_after_recoverable_error() {
        let source = "form 0\nbroken {\n source: test/source\n  ?? nope\n sink: test/sink\n}\n";
        let document = parse_document(source, &catalog());
        let diagnostic = document
            .diagnostics
            .first()
            .expect("invalid statement is diagnosed");

        assert_eq!(document.round_trip(), source);
        assert_eq!(diagnostic.code, "CND-FRM-013");
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            "?? nope"
        );
        assert_eq!(diagnostic.span.line, 4);
        assert_eq!(diagnostic.span.column, 3);
        assert!(document.checked_form.is_none());
        assert!(document
            .tokens
            .iter()
            .any(|token| token.text == "test/sink"));
    }

    #[test]
    fn missing_close_is_diagnosed_at_eof_without_losing_source() {
        let source = "form 0\nopen {\n source: test/source\n";
        let document = parse_document(source, &catalog());
        let diagnostic = document
            .diagnostics
            .first()
            .expect("missing close is diagnosed");

        assert_eq!(diagnostic.code, "CND-FRM-004");
        assert_eq!(diagnostic.span.start, source.len());
        assert_eq!(diagnostic.span.end, source.len());
        assert_eq!(document.round_trip(), source);
    }

    #[test]
    fn lossless_document_preserves_distinct_source_and_checked_identities() {
        let baseline_source =
            "form 0\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let spelling_source = "# layout only\nform 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
        let baseline = parse_document(baseline_source, &catalog());
        let spelling = parse_document(spelling_source, &catalog());
        let baseline = baseline.checked().expect("baseline checks");
        let spelling = spelling.checked().expect("spelling checks");

        assert_ne!(baseline.source_document_id, spelling.source_document_id);
        assert_eq!(baseline.checked_form_id, spelling.checked_form_id);
        assert_eq!(baseline.expanded_form_id, spelling.expanded_form_id);
    }

    #[test]
    fn lossless_document_enforces_source_and_token_bounds() {
        let oversized_source = " ".repeat(MAXIMUM_FORM_SOURCE_BYTES + 1);
        let source_document = parse_document(&oversized_source, &catalog());
        assert_eq!(source_document.diagnostics[0].code, "CND-FRM-014");
        assert!(matches!(
            parse(&oversized_source, &catalog()),
            Err(FormError::SourceLimitExceeded)
        ));

        let token_heavy_source = "x ".repeat(MAXIMUM_FORM_TOKENS + 1);
        assert!(token_heavy_source.len() < MAXIMUM_FORM_SOURCE_BYTES);
        let token_document = parse_document(&token_heavy_source, &catalog());
        assert_eq!(token_document.diagnostics[0].code, "CND-FRM-015");
        assert_eq!(token_document.round_trip(), token_heavy_source);
        assert!(token_document.checked_form.is_none());
    }

    #[test]
    fn parses_catalog_supplied_kinds_and_ports() {
        let form = parse(
            "form 0\n\ndemo {\n source: test/source\n sink: test/sink\n source.count = 3\n source > sink\n}\n",
            &catalog(),
        )
        .expect("form parses");
        assert_eq!(form.operations[0].kind_id.as_str(), "test/sink");
        assert_eq!(form.operations[1].kind_id.as_str(), "test/source");
        assert_eq!(form.connections[0].source_port_id.as_str(), "out");
        assert_eq!(form.connections[0].sink_port_id.as_str(), "in");
    }

    #[test]
    fn rejects_kinds_absent_from_catalog() {
        let error = parse("form 0\n\nbad {\n op: missing/kind\n}\n", &catalog())
            .expect_err("unknown kind fails");
        assert!(error.to_string().contains("missing/kind"));
    }

    #[test]
    fn enforces_catalog_supplied_configuration_rules() {
        let error = parse(
            "form 0\n\ndemo {\n source: test/source\n source.count = 5\n}\n",
            &catalog(),
        )
        .expect_err("out-of-range catalog value fails");
        assert!(matches!(error, super::FormError::InvalidConfiguration(_)));
    }

    #[test]
    fn checks_authored_exports_against_real_connections() {
        let form = parse(
            "form 0\n\ncomposite {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite = source.out -> sink.in\n}\n",
            &catalog(),
        )
        .expect("authored export parses");
        assert_eq!(form.exports.len(), 1);
        assert_eq!(form.exports[0].capability_id.as_str(), "run");
        assert_eq!(form.exports[0].kind_id.as_str(), "test/composite");
        assert_eq!(form.exports[0].value_kind.as_str(), "test/value");

        let error = parse(
            "form 0\n\nbad {\n source: test/source\n sink: test/sink\n export run: test/composite = source.out -> sink.in\n}\n",
            &catalog(),
        )
        .expect_err("an export cannot invent a connection");
        assert!(matches!(error, super::FormError::InvalidExport(_)));
    }

    #[test]
    fn checked_export_is_the_only_source_of_a_parent_kind_boundary() {
        let source = "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite = source.out -> sink.in\n}\n";
        let child = parse(source, &catalog()).expect("child checks");
        let capability_id = CapabilityId::from("run");
        let boundary = child
            .export_boundary(&capability_id)
            .expect("authored export derives a boundary");

        assert_eq!(boundary.kind_id.as_str(), "test/composite");
        assert_eq!(boundary.inputs, Vec::new());
        assert_eq!(boundary.outputs.len(), 1);
        assert_eq!(boundary.outputs[0].port_id.as_str(), "out");
        assert_eq!(boundary.value_kind.as_str(), "test/value");
        assert!(child
            .export_boundary(&CapabilityId::from("invented"))
            .is_err());

        let mut parent_catalog = catalog();
        let installed = parent_catalog
            .insert_export(&child, &capability_id)
            .expect("checked boundary installs");
        let parent = parse(
            "form 0\nparent {\n child: test/composite\n sink: test/sink\n child.out -> sink.in\n}\n",
            &parent_catalog,
        )
        .expect("ordinary parent cord checks");
        assert_eq!(
            parent.operations[0].kind_contract_revision,
            installed.kind_contract_revision
        );
        assert_eq!(parent.connections[0].source_port_id.as_str(), "out");

        let changed = parse(
            "form 0\nchild {\n source: test/source\n sink: test/sink\n source.count = 2\n source > sink\n export run: test/composite = source.out -> sink.in\n}\n",
            &catalog(),
        )
        .expect("semantic change checks");
        assert_eq!(
            boundary.kind_contract_revision,
            changed
                .export_boundary(&capability_id)
                .expect("changed export derives a boundary")
                .kind_contract_revision
        );
        assert_ne!(child.checked_form_id, changed.checked_form_id);
        assert_ne!(child.expanded_form_id, changed.expanded_form_id);
    }

    #[test]
    fn duplicate_export_capabilities_are_rejected() {
        let error = parse(
            "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite = source.out -> sink.in\n export run: test/other = source.out -> sink.in\n}\n",
            &catalog(),
        )
        .expect_err("one capability cannot name two boundaries");
        assert!(matches!(error, FormError::InvalidExport(_)));
    }

    #[test]
    fn inline_nested_form_uses_the_same_checked_boundary_as_a_standalone_form() {
        let standalone_source = "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite = source.out -> sink.in\n}\n";
        let nested_source = "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n final: test/sink\n child.out -> final.in\n}\n";
        let standalone = parse(standalone_source, &catalog()).expect("standalone child checks");
        let parent = parse(nested_source, &catalog()).expect("inline nested form checks");
        let nested = &parent.nested_forms[0];
        let capability_id = CapabilityId::from("run");

        assert_eq!(nested.operation_id.as_str(), "child");
        assert_eq!(nested.export_capability_id, capability_id);
        assert_eq!(nested.form.checked_form_id, standalone.checked_form_id);
        assert_eq!(nested.form.expanded_form_id, standalone.expanded_form_id);
        assert_ne!(
            nested.form.source_document_id,
            standalone.source_document_id
        );
        assert_eq!(
            nested
                .form
                .export_boundary(&capability_id)
                .expect("nested boundary checks"),
            standalone
                .export_boundary(&capability_id)
                .expect("standalone boundary checks")
        );
        assert_eq!(parent.connections.len(), 1);
        assert_eq!(parent.connections[0].source_operation_id.as_str(), "child");
        assert_eq!(parent.connections[0].source_port_id.as_str(), "out");
        assert_eq!(parent.connections[0].sink_operation_id.as_str(), "final");
    }

    #[test]
    fn parent_expanded_identity_binds_hidden_child_semantics_not_checked_boundary() {
        let baseline = parse(
            "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n final: test/sink\n child.out -> final.in\n}\n",
            &catalog(),
        )
        .expect("baseline nested parent checks");
        let changed = parse(
            "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n final: test/sink\n child.out -> final.in\n}\n",
            &catalog(),
        )
        .expect("changed nested parent checks");

        assert_ne!(
            baseline.nested_forms[0].form.checked_form_id,
            changed.nested_forms[0].form.checked_form_id
        );
        assert_eq!(
            baseline.operations[0].kind_contract_revision,
            changed.operations[0].kind_contract_revision
        );
        assert_eq!(baseline.checked_form_id, changed.checked_form_id);
        assert_ne!(baseline.expanded_form_id, changed.expanded_form_id);
        baseline
            .validate_identities()
            .expect("baseline identities validate");
        changed
            .validate_identities()
            .expect("changed identities validate");
    }

    #[test]
    fn nested_expansion_paths_are_canonical_and_substitution_fails_closed() {
        let baseline = parse(
            "form 0\nparent {\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("two nested paths check");
        let source_reordered = parse(
            "form 0\nparent {\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("source-reordered nested paths check");
        let implementations_swapped = parse(
            "form 0\nparent {\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("swapped nested implementations check");

        assert_eq!(baseline.checked_form_id, source_reordered.checked_form_id);
        assert_eq!(baseline.expanded_form_id, source_reordered.expanded_form_id);
        assert_eq!(
            baseline.checked_form_id,
            implementations_swapped.checked_form_id
        );
        assert_ne!(
            baseline.expanded_form_id,
            implementations_swapped.expanded_form_id
        );
        assert_eq!(baseline.nested_forms[0].operation_id.as_str(), "left");
        assert_eq!(baseline.nested_forms[1].operation_id.as_str(), "right");

        let mut omitted = baseline.clone();
        omitted.nested_forms.remove(0);
        assert!(matches!(
            omitted.validate_identities(),
            Err(FormError::InvalidIdentity(_))
        ));

        let mut duplicated = baseline.clone();
        duplicated
            .nested_forms
            .push(duplicated.nested_forms[0].clone());
        assert!(matches!(
            duplicated.validate_identities(),
            Err(FormError::InvalidIdentity(_))
        ));

        let mut reordered = baseline.clone();
        reordered.nested_forms.swap(0, 1);
        assert!(matches!(
            reordered.validate_identities(),
            Err(FormError::InvalidIdentity(_))
        ));

        let mut substituted = baseline;
        substituted.nested_forms[0].form = implementations_swapped.nested_forms[0].form.clone();
        assert!(matches!(
            substituted.validate_identities(),
            Err(FormError::InvalidIdentity(_))
        ));
    }

    #[test]
    fn nested_errors_keep_the_outer_document_and_exact_inner_span() {
        let source = "form 0\nparent {\n child: run {\n  source: test/source\n  ?? inner error\n  sink: test/sink\n  source > sink\n  export run: test/composite = source.out -> sink.in\n }\n}\n";
        let document = parse_document(source, &catalog());
        let diagnostic = &document.diagnostics[0];

        assert_eq!(document.round_trip(), source);
        assert_eq!(diagnostic.code, "CND-FRM-013");
        assert_eq!(diagnostic.span.line, 5);
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            "?? inner error"
        );
        assert!(document.checked_form.is_none());
        assert!(document
            .tokens
            .iter()
            .any(|token| token.text == "test/sink"));
    }

    #[test]
    fn inline_nesting_has_a_hard_depth_ceiling() {
        let mut source = String::from("form 0\nroot {\n");
        for depth in 0..=MAXIMUM_FORM_NESTING_DEPTH {
            source.push_str(&format!("n{depth}: run {{\n"));
        }
        let document = parse_document(&source, &catalog());

        assert_eq!(document.diagnostics[0].code, "CND-FRM-016");
        assert!(document.checked_form.is_none());
    }
}
