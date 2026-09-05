#![no_std]

#[macro_use]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod prelude {
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    CapabilityId, CheckedFormId, ConfigurationEntry, ConfigurationValue, ExpandedFormId,
    FormIdentity, GearId, KindContractRevision, KindId, PortDescriptor, PortDirection, PortId,
    SourceDocumentId,
};
use sha2::{Digest, Sha256};

mod back_catalog;
mod canonical_expansion;
mod checked_syntax;
mod diagnostic;
mod functional_face;
mod structured_expression;
mod structured_selector;
mod structured_startup;
mod surface_lex;
mod surface_parser;
pub mod syntax;
mod syntax_check;
mod syntax_highlight;
mod syntax_identity;
mod text_value;
mod value_type;

pub use back_catalog::*;
pub use canonical_expansion::*;
pub use checked_syntax::*;
pub use diagnostic::*;
pub use structured_startup::*;
pub use syntax::*;
pub use syntax_highlight::*;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGear {
    pub gear_id: GearId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub startup_parameters: Vec<conduit_core::FaceStartupParameter>,
    pub shorthand: Option<(PortId, PortId)>,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<ConfigurationEntry>,
    pub pool_references: Vec<conduit_core::SharedPoolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedConnection {
    pub source_gear_id: GearId,
    pub source_port_id: PortId,
    pub sink_gear_id: GearId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    pub temporal: conduit_core::PortTemporal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedForm {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub name: String,
    pub gears: Vec<CheckedGear>,
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
            if pair[0].gear_id >= pair[1].gear_id {
                return Err(FormError::InvalidIdentity(
                    "nested expansion rows are not unique canonical paths".into(),
                ));
            }
        }
        for nested in &self.nested_forms {
            nested.form.validate_identities()?;
            let gear = self
                .gears
                .iter()
                .find(|gear| gear.gear_id == nested.gear_id)
                .ok_or_else(|| {
                    FormError::InvalidIdentity(format!(
                        "nested expansion path '{}' has no checked gear",
                        nested.gear_id.as_str()
                    ))
                })?;
            let boundary = nested
                .form
                .export_boundary_unvalidated(&nested.export_capability_id)?;
            let definition = boundary.kind_definition();
            if gear.kind_id != definition.kind_id
                || gear.kind_contract_revision != definition.kind_contract_revision
                || gear.inputs != definition.inputs
                || gear.outputs != definition.outputs
                || !gear.configuration.is_empty()
            {
                return Err(FormError::InvalidIdentity(format!(
                    "nested expansion path '{}' differs from its selected export",
                    nested.gear_id.as_str()
                )));
            }
        }

        let expected_checked =
            checked_form_id(&self.name, &self.gears, &self.connections, &self.exports);
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
        let mut export = self
            .exports
            .iter()
            .find(|export| &export.capability_id == capability_id)
            .cloned()
            .or_else(|| (self.exports.len() == 1).then(|| self.exports[0].clone()))
            .ok_or_else(|| {
                FormError::InvalidExport(format!(
                    "checked form has no authored capability '{}'",
                    capability_id.as_str()
                ))
            })?;
        export.capability_id = capability_id.clone();
        validate_export_faces(&export, &self.gears)?;
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
    pub gear_id: GearId,
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
    pub internal_gear_id: GearId,
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
    I64Range { minimum: i64, maximum: i64 },
    DurationMillis { minimum: u64, maximum: u64 },
    TextBytes { maximum: u32 },
    TextOneOf { values: Vec<String> },
    Structured { profile: KindId },
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

    /// Derives the startup names and defaults needed to check canonical source.
    /// Structured startup types still require an explicitly assembled
    /// [`StartupCatalog`].
    pub fn startup_catalog(&self) -> Result<StartupCatalog, String> {
        let mut startup = StartupCatalog::new();
        for definition in self.kinds.values() {
            startup.insert(KindSignature {
                kind: definition.kind_id.as_str().to_string(),
                startup_parameters: definition
                    .configuration
                    .iter()
                    .map(|field| StartupParameterSignature {
                        name: field.key.clone(),
                        value_type: match &field.default_value {
                            ConfigurationValue::Bool(_) => "Boolean",
                            ConfigurationValue::U64(_) => "Count",
                            ConfigurationValue::I64(_) => "Scalar",
                            ConfigurationValue::Text(_) => "Text",
                            ConfigurationValue::Structured(_) => "Structured",
                        }
                        .into(),
                        default: Some(render_value(&field.default_value)),
                    })
                    .collect(),
            })?;
        }
        Ok(startup)
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    SourceLimitExceeded,
    TokenLimitExceeded,
    IncompleteForm,
    MissingBlockEnd,
    DuplicateKind(String),
    InvalidExport(String),
    InvalidIdentity(String),
    InvalidSyntax(String),
}

impl core::fmt::Display for FormError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceLimitExceeded => write!(
                f,
                "form source exceeds the {MAXIMUM_FORM_SOURCE_BYTES}-byte limit"
            ),
            Self::TokenLimitExceeded => write!(
                f,
                "form source exceeds the {MAXIMUM_FORM_TOKENS}-token limit"
            ),
            Self::IncompleteForm => write!(f, "incomplete form"),
            Self::MissingBlockEnd => write!(f, "expected closing '}}' at end of form"),
            Self::DuplicateKind(kind) => write!(f, "duplicate profile kind '{kind}'"),
            Self::InvalidExport(message) => write!(f, "invalid export: {message}"),
            Self::InvalidIdentity(message) => write!(f, "invalid form identity: {message}"),
            Self::InvalidSyntax(message) => write!(f, "invalid canonical form syntax: {message}"),
        }
    }
}

impl core::error::Error for FormError {}

/// Parses the canonical `form NAME (...) { ... }` surface without performing
/// catalog lookup or semantic lowering.
pub fn parse_syntax_document(source: &str) -> SyntaxDocument {
    surface_parser::parse_surface(source)
}

/// Checks immutable startup bindings in canonical Form syntax without
/// recursively expanding forms or producing planner/runtime input.
pub fn check_syntax_document(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    syntax_check::check_document(document, catalog)
}

pub fn parse(source: &str, catalog: &ProfileCatalog) -> Result<CheckedForm, FormError> {
    let startup = catalog
        .startup_catalog()
        .map_err(FormError::InvalidSyntax)?;
    parse_with_startup(source, &startup, catalog)
}

pub fn parse_with_startup(
    source: &str,
    startup: &StartupCatalog,
    catalog: &ProfileCatalog,
) -> Result<CheckedForm, FormError> {
    let syntax = parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(FormError::InvalidSyntax(diagnostic.message.clone()));
    }
    let checked = check_syntax_document(&syntax, startup)
        .map_err(|diagnostic| FormError::InvalidSyntax(diagnostic.message))?;
    let entry = checked
        .forms
        .last()
        .ok_or(FormError::IncompleteForm)?
        .name
        .clone();
    let authoring = expand_canonical_form_for_authoring(&checked, &entry, catalog)
        .map_err(|diagnostic| FormError::InvalidSyntax(diagnostic.message))?;
    let expanded = authoring.expanded;
    let input_faces = authoring
        .input_bindings
        .iter()
        .map(|binding| CheckedCompositeFace {
            external_port: authoring
                .face
                .inputs()
                .iter()
                .find(|port| port.port_id == binding.face_port_id)
                .expect("authoring input binding names a checked face port")
                .clone(),
            internal_gear_id: binding.gear_id.clone(),
            internal_port_id: binding.gear_port_id.clone(),
            terminal: CompositeFaceTerminal::Independent,
        })
        .collect::<Vec<_>>();
    let output_faces = authoring
        .output_bindings
        .iter()
        .map(|binding| CheckedCompositeFace {
            external_port: authoring
                .face
                .outputs()
                .iter()
                .find(|port| port.port_id == binding.face_port_id)
                .expect("authoring output binding names a checked face port")
                .clone(),
            internal_gear_id: binding.gear_id.clone(),
            internal_port_id: binding.gear_port_id.clone(),
            terminal: CompositeFaceTerminal::Independent,
        })
        .collect::<Vec<_>>();
    let exports = if input_faces.is_empty() && output_faces.is_empty() {
        Vec::new()
    } else {
        vec![CheckedExport {
            capability_id: CapabilityId::from(entry.rsplit('/').next().unwrap_or(&entry)),
            kind_id: KindId::from(entry.as_str()),
            input_faces,
            output_faces,
        }]
    };
    let checked_form_id = checked_form_id(
        &expanded.name,
        &expanded.gears,
        &expanded.connections,
        &exports,
    );
    let expanded_form_id = expanded_form_id(&checked_form_id, &[]);
    Ok(CheckedForm {
        source_document_id: expanded.source_document_id,
        checked_form_id,
        expanded_form_id,
        name: expanded.name,
        gears: expanded.gears,
        connections: expanded.connections,
        exports,
        nested_forms: Vec::new(),
    })
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
            let mut quote = None;
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
                if let Some(active) = quote {
                    if next == active && !escaped {
                        quote = None;
                    }
                    escaped = next == '\\' && !escaped;
                    if next != '\\' {
                        escaped = false;
                    }
                } else if matches!(next, '\'' | '"') {
                    quote = Some(next);
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
        FormError::IncompleteForm => "CND-FRM-002",
        FormError::MissingBlockEnd => "CND-FRM-004",
        FormError::DuplicateKind(_) => "CND-FRM-006",
        FormError::InvalidExport(_) => "CND-FRM-012",
        FormError::SourceLimitExceeded => "CND-FRM-014",
        FormError::TokenLimitExceeded => "CND-FRM-015",
        FormError::InvalidIdentity(_) => "CND-FRM-018",
        FormError::InvalidSyntax(_) => "CND-FRM-019",
    };
    FormDiagnostic {
        code,
        span,
        message: error.to_string(),
    }
}

fn validate_export_faces(export: &CheckedExport, gears: &[CheckedGear]) -> Result<(), FormError> {
    let mut names = BTreeSet::new();
    for face in export.input_faces.iter().chain(&export.output_faces) {
        if !names.insert(face.external_port.port_id.clone()) {
            return Err(FormError::InvalidExport(format!(
                "duplicate face name '{}'",
                face.external_port.port_id.as_str()
            )));
        }
    }
    for (direction, faces) in [
        (PortDirection::Input, &export.input_faces),
        (PortDirection::Output, &export.output_faces),
    ] {
        for face in faces {
            if face.external_port.direction != direction {
                return Err(FormError::InvalidExport(
                    "face direction differs from its export collection".into(),
                ));
            }
            let gear = gears
                .iter()
                .find(|gear| gear.gear_id == face.internal_gear_id)
                .ok_or_else(|| FormError::InvalidExport("face names a missing Gear".into()))?;
            let endpoint = match direction {
                PortDirection::Input => &gear.inputs,
                PortDirection::Output => &gear.outputs,
            }
            .iter()
            .find(|port| port.port_id == face.internal_port_id)
            .ok_or_else(|| {
                FormError::InvalidExport("face names a missing or wrongly directed Port".into())
            })?;
            if endpoint.value_kind != face.external_port.value_kind
                || face.terminal != CompositeFaceTerminal::Independent
            {
                return Err(FormError::InvalidExport(
                    "face contract differs from its internal endpoint".into(),
                ));
            }
        }
    }
    Ok(())
}

fn canonical_form_text(
    name: &str,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> String {
    let mut text = format!("form:{name}\n");
    for gear in gears {
        text.push_str(&format!(
            "op:{}:{}:{}|",
            gear.gear_id.as_str(),
            gear.kind_id.as_str(),
            gear.kind_contract_revision.as_str()
        ));
        for port in gear.inputs.iter().chain(&gear.outputs) {
            let direction = match port.direction {
                conduit_core::PortDirection::Input => "input",
                conduit_core::PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "port:{}:{}:{}:{}|",
                port.port_id.as_str(),
                port.value_kind.as_str(),
                direction,
                port.temporal.as_str()
            ));
        }
        for entry in &gear.configuration {
            text.push_str(&format!(
                "cfg:{}={}|",
                entry.key,
                render_value(&entry.value)
            ));
        }
    }
    for connection in connections {
        text.push_str(&format!(
            "conn:{}:{}->{}:{}:{}|",
            connection.source_gear_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_gear_id.as_str(),
            connection.sink_port_id.as_str(),
            connection.temporal.as_str()
        ));
    }
    for export in exports {
        text.push_str(&format!(
            "export:{}:{}|",
            export.capability_id.as_str(),
            export.kind_id.as_str(),
        ));
        for face in export.input_faces.iter().chain(&export.output_faces) {
            let direction = match face.external_port.direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "face:{direction}:{}:{}:{}={}:{}:terminal-independent|",
                face.external_port.port_id.as_str(),
                face.external_port.value_kind.as_str(),
                face.external_port.temporal.as_str(),
                face.internal_gear_id.as_str(),
                face.internal_port_id.as_str(),
            ));
        }
    }
    text
}

fn checked_form_id(
    name: &str,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> CheckedFormId {
    CheckedFormId::from(hash_string(&canonical_form_text(
        name,
        gears,
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
        push_identity_field(&mut canonical, nested.gear_id.as_str());
        push_identity_field(&mut canonical, nested.export_capability_id.as_str());
        push_identity_field(&mut canonical, nested.form.expanded_form_id.as_str());
    }
    ExpandedFormId::from(hash_string(&canonical))
}

fn exported_contract_revision(
    kind_id: &KindId,
    inputs: &[CheckedCompositeFace],
    outputs: &[CheckedCompositeFace],
) -> KindContractRevision {
    let mut canonical = String::from("checked-export-contract:");
    push_identity_field(&mut canonical, kind_id.as_str());
    for (direction, faces) in [("input", inputs), ("output", outputs)] {
        for face in faces {
            push_identity_field(&mut canonical, direction);
            push_identity_field(&mut canonical, face.external_port.port_id.as_str());
            push_identity_field(&mut canonical, face.external_port.value_kind.as_str());
            push_identity_field(&mut canonical, face.external_port.temporal.as_str());
            push_identity_field(
                &mut canonical,
                match face.terminal {
                    CompositeFaceTerminal::Independent => "independent",
                    CompositeFaceTerminal::Coupled => "coupled",
                },
            );
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
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Text(value) => format!("{value:?}"),
        ConfigurationValue::Structured(value) => alloc::format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
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
mod surface_tests;

#[cfg(test)]
mod syntax_check_tests;

#[cfg(test)]
mod canonical_expansion_tests;
