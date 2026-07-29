//! Hosted owned diagnostics, lossless JSON, source-safe fixes, and terminal rendering.

use std::fmt;

pub use conduit_core::DIAGNOSTIC_SCHEMA_VERSION;
use conduit_core::{
    Diagnostic, DiagnosticArgument, DiagnosticArgumentValue, DiagnosticContractError,
    DiagnosticEdit, DiagnosticFix, DiagnosticRelated, DiagnosticSeverity, DiagnosticSpan,
    FixApplicability, ImplementationError, PlanValidationError, ValidationError,
};
use conduit_panel::{ModuleResolutionError, ParseError, SourceSchemaError, SourceSpan};
use conduit_runtime::{LoweringDiagnostic, ResolutionError, RuntimeError};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Owned exact source extent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnosticSpan {
    pub document_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u64,
    pub column: u64,
    pub end_line: u64,
    pub end_column: u64,
}

/// One related span or subject.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnosticRelated {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<OwnedDiagnosticSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Sensitivity-safe argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnedDiagnosticArgumentValue {
    Public {
        text: String,
    },
    Redacted {
        sensitivity: String,
        value_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        byte_len: Option<u64>,
    },
}

/// Named structured argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnosticArgument {
    pub name: String,
    pub value: OwnedDiagnosticArgumentValue,
}

/// Confidence boundary for a fix proposal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnedFixApplicability {
    MachineApplicable,
    MaybeIncorrect,
}

/// One guarded edit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnosticEdit {
    pub document_id: String,
    pub precondition_hash: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub replacement: String,
}

/// One unapplied source fix.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnosticFix {
    pub id: String,
    pub message: String,
    pub applicability: OwnedFixApplicability,
    pub edits: Vec<OwnedDiagnosticEdit>,
}

/// Stable severity spelling.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnedDiagnosticSeverity {
    Error,
    Warning,
    Note,
}

/// Complete versioned owned diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDiagnostic {
    pub schema_version: u32,
    pub code: String,
    pub severity: OwnedDiagnosticSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<OwnedDiagnosticSpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<OwnedDiagnosticRelated>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<OwnedDiagnosticArgument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<OwnedDiagnosticFix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<String>,
}

impl OwnedDiagnostic {
    /// Validates by borrowing the allocator-free core contract.
    pub fn validate(&self) -> Result<(), DiagnosticContractError> {
        let related: Vec<_> = self
            .related
            .iter()
            .map(|related| DiagnosticRelated {
                label: &related.label,
                span: related.span.as_ref().map(core_span),
                subject: related.subject.as_deref(),
            })
            .collect();
        let arguments: Vec<_> = self
            .arguments
            .iter()
            .map(|argument| DiagnosticArgument {
                name: &argument.name,
                value: match &argument.value {
                    OwnedDiagnosticArgumentValue::Public { text } => {
                        DiagnosticArgumentValue::Public(text)
                    }
                    OwnedDiagnosticArgumentValue::Redacted {
                        sensitivity,
                        value_type,
                        byte_len,
                    } => DiagnosticArgumentValue::Redacted {
                        sensitivity,
                        value_type,
                        byte_len: *byte_len,
                    },
                },
            })
            .collect();
        let edit_storage: Vec<Vec<_>> = self
            .fixes
            .iter()
            .map(|fix| {
                fix.edits
                    .iter()
                    .map(|edit| DiagnosticEdit {
                        document_id: &edit.document_id,
                        precondition_hash: &edit.precondition_hash,
                        byte_start: edit.byte_start,
                        byte_end: edit.byte_end,
                        replacement: &edit.replacement,
                    })
                    .collect()
            })
            .collect();
        let fixes: Vec<_> = self
            .fixes
            .iter()
            .zip(&edit_storage)
            .map(|(fix, edits)| DiagnosticFix {
                id: &fix.id,
                message: &fix.message,
                applicability: match fix.applicability {
                    OwnedFixApplicability::MachineApplicable => FixApplicability::MachineApplicable,
                    OwnedFixApplicability::MaybeIncorrect => FixApplicability::MaybeIncorrect,
                },
                edits,
            })
            .collect();
        let notes: Vec<_> = self.notes.iter().map(String::as_str).collect();
        let causes: Vec<_> = self.causes.iter().map(String::as_str).collect();
        Diagnostic {
            schema_version: self.schema_version,
            code: &self.code,
            severity: match self.severity {
                OwnedDiagnosticSeverity::Error => DiagnosticSeverity::Error,
                OwnedDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
                OwnedDiagnosticSeverity::Note => DiagnosticSeverity::Note,
            },
            message: &self.message,
            primary: self.primary.as_ref().map(core_span),
            related: &related,
            arguments: &arguments,
            notes: &notes,
            help: self.help.as_deref(),
            fixes: &fixes,
            semantic_path: self.semantic_path.as_deref(),
            causes: &causes,
        }
        .validate()
    }

    /// Encodes stable compact JSON after validation.
    pub fn to_json(&self) -> Result<String, DiagnosticJsonError> {
        self.validate().map_err(DiagnosticJsonError::Contract)?;
        serde_json::to_string(self).map_err(DiagnosticJsonError::Json)
    }

    /// Decodes lossless JSON and rejects unsupported or malformed structure.
    pub fn from_json(input: &str) -> Result<Self, DiagnosticJsonError> {
        let diagnostic: Self = serde_json::from_str(input).map_err(DiagnosticJsonError::Json)?;
        diagnostic
            .validate()
            .map_err(DiagnosticJsonError::Contract)?;
        Ok(diagnostic)
    }
}

fn core_span(span: &OwnedDiagnosticSpan) -> DiagnosticSpan<'_> {
    DiagnosticSpan {
        document_id: &span.document_id,
        content_hash: span.content_hash.as_deref(),
        byte_start: span.byte_start,
        byte_end: span.byte_end,
        line: span.line,
        column: span.column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

/// Diagnostic JSON failure.
#[derive(Debug)]
pub enum DiagnosticJsonError {
    Contract(DiagnosticContractError),
    Json(serde_json::Error),
}

impl fmt::Display for DiagnosticJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "invalid diagnostic contract: {error:?}"),
            Self::Json(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for DiagnosticJsonError {}

/// Exact source bytes used only for hosted presentation and fix freshness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSource {
    pub document_id: String,
    pub content_hash: String,
    pub bytes: Vec<u8>,
}

impl DiagnosticSource {
    #[must_use]
    pub fn new(document_id: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        let bytes = bytes.into();
        Self {
            document_id: document_id.into(),
            content_hash: source_hash(&bytes),
            bytes,
        }
    }
}

/// Terminal color mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalColor {
    Never,
    Always,
}

/// Terminal detail level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalVerbosity {
    Concise,
    Verbose,
}

/// Renders stable terminal text without changing diagnostic data.
#[must_use]
pub fn render_terminal(
    diagnostic: &OwnedDiagnostic,
    sources: &[DiagnosticSource],
    color: TerminalColor,
    verbosity: TerminalVerbosity,
) -> String {
    let mut output = String::new();
    let severity = match diagnostic.severity {
        OwnedDiagnosticSeverity::Error => "error",
        OwnedDiagnosticSeverity::Warning => "warning",
        OwnedDiagnosticSeverity::Note => "note",
    };
    if color == TerminalColor::Always {
        output.push_str("\u{1b}[1;31m");
    }
    output.push_str(severity);
    output.push('[');
    output.push_str(&diagnostic.code);
    output.push(']');
    if color == TerminalColor::Always {
        output.push_str("\u{1b}[0m");
    }
    output.push_str(": ");
    output.push_str(&diagnostic.message);
    output.push('\n');
    if let Some(primary) = &diagnostic.primary {
        render_span(&mut output, primary, sources, color, "-->");
    }
    if verbosity == TerminalVerbosity::Verbose {
        if let Some(path) = &diagnostic.semantic_path {
            output.push_str("path: ");
            output.push_str(path);
            output.push('\n');
        }
        for related in &diagnostic.related {
            output.push_str("related: ");
            output.push_str(&related.label);
            if let Some(subject) = &related.subject {
                output.push_str(" (");
                output.push_str(subject);
                output.push(')');
            }
            output.push('\n');
            if let Some(span) = &related.span {
                render_span(&mut output, span, sources, color, ":::");
            }
        }
        for argument in &diagnostic.arguments {
            output.push_str("argument ");
            output.push_str(&argument.name);
            output.push_str(": ");
            match &argument.value {
                OwnedDiagnosticArgumentValue::Public { text } => output.push_str(text),
                OwnedDiagnosticArgumentValue::Redacted { .. } => output.push_str("[REDACTED]"),
            }
            output.push('\n');
        }
        for note in &diagnostic.notes {
            output.push_str("note: ");
            output.push_str(note);
            output.push('\n');
        }
        for cause in &diagnostic.causes {
            output.push_str("caused by: ");
            output.push_str(cause);
            output.push('\n');
        }
    }
    if let Some(help) = &diagnostic.help {
        output.push_str("help: ");
        output.push_str(help);
        output.push('\n');
    }
    for fix in &diagnostic.fixes {
        output.push_str("fix[");
        output.push_str(&fix.id);
        output.push_str("] (");
        output.push_str(match fix.applicability {
            OwnedFixApplicability::MachineApplicable => "machine-applicable",
            OwnedFixApplicability::MaybeIncorrect => "maybe-incorrect",
        });
        output.push_str("): ");
        output.push_str(&fix.message);
        output.push('\n');
    }
    output
}

fn render_span(
    output: &mut String,
    span: &OwnedDiagnosticSpan,
    sources: &[DiagnosticSource],
    color: TerminalColor,
    marker: &str,
) {
    if color == TerminalColor::Always {
        output.push_str("\u{1b}[1;34m");
    }
    output.push_str(marker);
    if color == TerminalColor::Always {
        output.push_str("\u{1b}[0m");
    }
    output.push(' ');
    output.push_str(&span.document_id);
    output.push(':');
    output.push_str(&span.line.to_string());
    output.push(':');
    output.push_str(&span.column.to_string());
    output.push_str(" (bytes ");
    output.push_str(&span.byte_start.to_string());
    output.push_str("..");
    output.push_str(&span.byte_end.to_string());
    output.push_str(")\n");
    let Some(source) = sources
        .iter()
        .find(|source| source.document_id == span.document_id)
    else {
        return;
    };
    if let Some(line) = source
        .bytes
        .split(|byte| *byte == b'\n')
        .nth(usize::try_from(span.line.saturating_sub(1)).unwrap_or(usize::MAX))
    {
        output.push_str(&span.line.to_string());
        output.push_str(" | ");
        output.push_str(&String::from_utf8_lossy(line));
        output.push('\n');
        output.push_str("  | ");
        output.push_str(&" ".repeat(usize::try_from(span.column.saturating_sub(1)).unwrap_or(0)));
        output.push_str(
            &"^".repeat(
                usize::try_from(if span.line == span.end_line {
                    span.end_column.saturating_sub(span.column).max(1)
                } else {
                    1
                })
                .unwrap_or(1),
            ),
        );
        output.push('\n');
    }
}

/// Fix freshness without source mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixStatus {
    Applicable,
    StalePrecondition,
    MissingDocument,
    InvalidRange,
}

#[must_use]
pub fn check_fix(fix: &OwnedDiagnosticFix, sources: &[DiagnosticSource]) -> FixStatus {
    for edit in &fix.edits {
        let Some(source) = sources
            .iter()
            .find(|source| source.document_id == edit.document_id)
        else {
            return FixStatus::MissingDocument;
        };
        if source.content_hash != edit.precondition_hash {
            return FixStatus::StalePrecondition;
        }
        if edit.byte_start > edit.byte_end
            || usize::try_from(edit.byte_end)
                .ok()
                .is_none_or(|end| end > source.bytes.len())
        {
            return FixStatus::InvalidRange;
        }
    }
    FixStatus::Applicable
}

/// Computes exact source content identity.
#[must_use]
pub fn source_hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Converts a parser failure into one structured diagnostic.
#[must_use]
pub fn from_parse_error(error: &ParseError, source: &DiagnosticSource) -> OwnedDiagnostic {
    let primary = span_at_location(source, error.line, error.column);
    let fixes = parse_fixes(error, source, &primary);
    base(error.code, &error.message, Some(primary), fixes)
}

/// Converts an explicit persisted source-schema selection failure.
#[must_use]
pub fn from_source_schema_error(error: &SourceSchemaError) -> OwnedDiagnostic {
    let mut diagnostic = base(error.code, &error.message, None, Vec::new());
    diagnostic.arguments.push(OwnedDiagnosticArgument {
        name: "schema_version".to_owned(),
        value: OwnedDiagnosticArgumentValue::Public {
            text: error.schema_version.to_string(),
        },
    });
    diagnostic
}

/// Converts module resolution failure and import causality.
#[must_use]
pub fn from_module_error(
    error: &ModuleResolutionError,
    sources: &[DiagnosticSource],
) -> OwnedDiagnostic {
    let primary = sources
        .iter()
        .find(|source| source.document_id == error.uri)
        .map(|source| span_at_location(source, 1, 1));
    let mut diagnostic = base(error.code, &error.message, primary, Vec::new());
    diagnostic.related = error
        .import_chain
        .iter()
        .map(|uri| OwnedDiagnosticRelated {
            label: "import chain".to_owned(),
            span: sources
                .iter()
                .find(|source| source.document_id == *uri)
                .map(|source| span_at_location(source, 1, 1)),
            subject: Some(uri.clone()),
        })
        .collect();
    diagnostic
}

/// Converts typed lowering failure while retaining semantic and source paths.
#[must_use]
pub fn from_lowering_error(
    error: &LoweringDiagnostic,
    sources: &[DiagnosticSource],
) -> OwnedDiagnostic {
    let primary = error.origin.as_ref().and_then(|origin| {
        sources
            .iter()
            .find(|source| source.document_id == origin.module_uri)
            .map(|source| span_from_source_span(source, origin.span))
    });
    let mut diagnostic = base(error.code, &error.message, primary, Vec::new());
    diagnostic.semantic_path = Some(error.semantic_path.clone());
    if let Some(expected) = &error.expected_contract {
        diagnostic.arguments.push(OwnedDiagnosticArgument {
            name: "expected_contract".to_owned(),
            value: OwnedDiagnosticArgumentValue::Public {
                text: expected.id.clone(),
            },
        });
    }
    diagnostic.help = lowering_help(error.code).map(str::to_owned);
    diagnostic.fixes = lowering_fixes(error, sources);
    diagnostic
}

/// Converts hosted resolution failure.
#[must_use]
pub fn from_resolution_error(error: &ResolutionError) -> OwnedDiagnostic {
    base(error.code, &error.message, None, Vec::new())
}

/// Converts execution failure without using arbitrary error-chain formatting.
#[must_use]
pub fn from_runtime_error(error: &RuntimeError) -> OwnedDiagnostic {
    base(error.code, &error.message, None, Vec::new())
}

/// Converts a host-neutral implementation contract violation.
#[must_use]
pub fn from_implementation_error(error: ImplementationError) -> OwnedDiagnostic {
    base(error.code(), &error.to_string(), None, Vec::new())
}

/// Source-aware context for a portable compatibility failure.
pub struct CompatibilityDiagnosticContext<'a> {
    pub cord: OwnedDiagnosticSpan,
    pub writer: OwnedDiagnosticSpan,
    pub reader: OwnedDiagnosticSpan,
    pub writer_contract: &'a str,
    pub reader_contract: &'a str,
    pub semantic_path: Option<&'a str>,
    pub cause_code: &'a str,
    pub known_adapter: Option<KnownAdapterFix<'a>>,
}

/// Explicitly known adapter edit. No adapter is guessed.
pub struct KnownAdapterFix<'a> {
    pub adapter_id: &'a str,
    pub edit: OwnedDiagnosticEdit,
}

/// Converts a portable graph validation failure with both port endpoints.
#[must_use]
pub fn from_validation_error(
    error: ValidationError,
    context: CompatibilityDiagnosticContext<'_>,
) -> OwnedDiagnostic {
    let mut diagnostic = base(
        error.code.as_str(),
        "writer port is not accepted by reader port",
        Some(context.cord),
        Vec::new(),
    );
    diagnostic.related = vec![
        OwnedDiagnosticRelated {
            label: "writer port".to_owned(),
            span: Some(context.writer),
            subject: Some(context.writer_contract.to_owned()),
        },
        OwnedDiagnosticRelated {
            label: "reader port".to_owned(),
            span: Some(context.reader),
            subject: Some(context.reader_contract.to_owned()),
        },
    ];
    diagnostic.arguments = vec![
        public_argument("writer_contract", context.writer_contract),
        public_argument("reader_contract", context.reader_contract),
    ];
    diagnostic.semantic_path = context.semantic_path.map(str::to_owned);
    diagnostic.causes.push(context.cause_code.to_owned());
    if let Some(adapter) = context.known_adapter {
        diagnostic.help = Some(format!(
            "insert the explicitly registered adapter `{}`",
            adapter.adapter_id
        ));
        diagnostic.fixes.push(OwnedDiagnosticFix {
            id: "insert-known-adapter".to_owned(),
            message: format!("insert `{}`", adapter.adapter_id),
            applicability: OwnedFixApplicability::MaybeIncorrect,
            edits: vec![adapter.edit],
        });
    }
    diagnostic
}

/// Source/path context for exact-plan validation.
pub struct PlanDiagnosticContext {
    pub primary: Option<OwnedDiagnosticSpan>,
    pub semantic_path: Option<String>,
}

/// Converts allocator-free plan validation output.
#[must_use]
pub fn from_plan_error(
    error: PlanValidationError,
    context: PlanDiagnosticContext,
) -> OwnedDiagnostic {
    let message = match error.code {
        conduit_core::PlanDiagnosticCode::Containment(reason) => {
            format!("administrative containment failed: {}", reason.as_str())
        }
        conduit_core::PlanDiagnosticCode::PolicyBudget(reason) => {
            format!("persistent policy budget failed: {}", reason.as_str())
        }
        _ => "exact execution plan validation failed".to_owned(),
    };
    let mut diagnostic = base(error.code.as_str(), &message, context.primary, Vec::new());
    diagnostic.semantic_path = context.semantic_path;
    diagnostic.arguments.push(public_argument(
        "collection",
        &format!("{:?}", error.collection),
    ));
    diagnostic
}

fn base(
    code: &str,
    message: &str,
    primary: Option<OwnedDiagnosticSpan>,
    fixes: Vec<OwnedDiagnosticFix>,
) -> OwnedDiagnostic {
    OwnedDiagnostic {
        schema_version: DIAGNOSTIC_SCHEMA_VERSION,
        code: code.to_owned(),
        severity: OwnedDiagnosticSeverity::Error,
        message: message.to_owned(),
        primary,
        related: Vec::new(),
        arguments: Vec::new(),
        notes: Vec::new(),
        help: None,
        fixes,
        semantic_path: None,
        causes: Vec::new(),
    }
}

fn public_argument(name: &str, text: &str) -> OwnedDiagnosticArgument {
    OwnedDiagnosticArgument {
        name: name.to_owned(),
        value: OwnedDiagnosticArgumentValue::Public {
            text: text.to_owned(),
        },
    }
}

fn span_at_location(source: &DiagnosticSource, line: usize, column: usize) -> OwnedDiagnosticSpan {
    let byte_start = byte_offset(&source.bytes, line, column);
    let byte_end = byte_start
        .checked_add(1)
        .filter(|end| *end <= source.bytes.len())
        .unwrap_or(byte_start);
    OwnedDiagnosticSpan {
        document_id: source.document_id.clone(),
        content_hash: Some(source.content_hash.clone()),
        byte_start: byte_start as u64,
        byte_end: byte_end as u64,
        line: line as u64,
        column: column as u64,
        end_line: line as u64,
        end_column: column.saturating_add(byte_end.saturating_sub(byte_start)) as u64,
    }
}

fn span_from_source_span(source: &DiagnosticSource, span: SourceSpan) -> OwnedDiagnosticSpan {
    OwnedDiagnosticSpan {
        document_id: source.document_id.clone(),
        content_hash: Some(source.content_hash.clone()),
        byte_start: byte_offset(&source.bytes, span.line, span.column) as u64,
        byte_end: byte_offset(&source.bytes, span.end_line, span.end_column) as u64,
        line: span.line as u64,
        column: span.column as u64,
        end_line: span.end_line as u64,
        end_column: span.end_column as u64,
    }
}

fn byte_offset(bytes: &[u8], line: usize, column: usize) -> usize {
    let mut line_start = 0;
    for _ in 1..line {
        let Some(relative_end) = bytes[line_start..].iter().position(|byte| *byte == b'\n') else {
            return bytes.len();
        };
        line_start += relative_end + 1;
    }
    let line_end = bytes[line_start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |relative| line_start + relative);
    let line_bytes = &bytes[line_start..line_end];
    let relative = std::str::from_utf8(line_bytes).map_or_else(
        |_| column.saturating_sub(1).min(line_bytes.len()),
        |text| {
            text.char_indices()
                .nth(column.saturating_sub(1))
                .map_or(text.len(), |(offset, _)| offset)
        },
    );
    line_start + relative
}

fn parse_fixes(
    error: &ParseError,
    source: &DiagnosticSource,
    primary: &OwnedDiagnosticSpan,
) -> Vec<OwnedDiagnosticFix> {
    let (id, message, start, end, replacement) = if error.message.contains("expected `->`") {
        (
            "insert-cord-arrow",
            "insert the missing cord arrow",
            primary.byte_start,
            primary.byte_start,
            "-> ",
        )
    } else if error.message.contains("unsupported panel version") {
        let version_start = source
            .bytes
            .windows(6)
            .position(|window| window == b"panel ")
            .map_or(primary.byte_start, |start| (start + 6) as u64);
        let version_end = source
            .bytes
            .get(usize::try_from(version_start).unwrap_or(source.bytes.len())..)
            .and_then(|rest| {
                rest.iter()
                    .position(|byte| !byte.is_ascii_digit())
                    .map(|length| version_start + length as u64)
            })
            .unwrap_or(primary.byte_end);
        (
            "use-panel-version-1",
            "replace the unsupported grammar version with version 1",
            version_start,
            version_end,
            "1",
        )
    } else if error.message.contains("trailing comma") {
        let comma = source
            .bytes
            .get(..usize::try_from(primary.byte_start).unwrap_or(0))
            .and_then(|prefix| prefix.iter().rposition(|byte| *byte == b','))
            .map_or(primary.byte_start.saturating_sub(1), |index| index as u64);
        (
            "remove-trailing-comma",
            "remove the trailing comma",
            comma,
            comma + 1,
            "",
        )
    } else {
        return Vec::new();
    };
    vec![OwnedDiagnosticFix {
        id: id.to_owned(),
        message: message.to_owned(),
        applicability: OwnedFixApplicability::MachineApplicable,
        edits: vec![OwnedDiagnosticEdit {
            document_id: source.document_id.clone(),
            precondition_hash: source.content_hash.clone(),
            byte_start: start,
            byte_end: end,
            replacement: replacement.to_owned(),
        }],
    }]
}

fn lowering_help(code: &str) -> Option<&'static str> {
    match code {
        "CND-LWR-002" => Some("remove the unknown field or declare it in the node contract"),
        "CND-LWR-004" => Some("supply the required field using its expected contract"),
        "CND-LWR-006" => Some("choose an integer within the expected contract bounds"),
        "CND-LWR-008" => Some("register the required domain type provider before lowering"),
        "CND-LWR-009" => Some("use an unresolved secret reference at a protected plan field"),
        _ => None,
    }
}

fn lowering_fixes(
    error: &LoweringDiagnostic,
    sources: &[DiagnosticSource],
) -> Vec<OwnedDiagnosticFix> {
    let Some(origin) = &error.origin else {
        return Vec::new();
    };
    let Some(source) = sources
        .iter()
        .find(|source| source.document_id == origin.module_uri)
    else {
        return Vec::new();
    };
    let span = span_from_source_span(source, origin.span);
    let (id, message, replacement) = match error.code {
        "CND-LWR-009" => (
            "replace-with-secret-reference",
            "replace protected value material with an unresolved binding",
            "secret(\"binding-name\")",
        ),
        _ => return Vec::new(),
    };
    vec![OwnedDiagnosticFix {
        id: id.to_owned(),
        message: message.to_owned(),
        applicability: OwnedFixApplicability::MaybeIncorrect,
        edits: vec![OwnedDiagnosticEdit {
            document_id: source.document_id.clone(),
            precondition_hash: source.content_hash.clone(),
            byte_start: span.byte_start,
            byte_end: span.byte_end,
            replacement: replacement.to_owned(),
        }],
    }]
}
