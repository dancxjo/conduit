use crate::{hash_string, FormDiagnostic, Span};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;
pub const MAXIMUM_DIAGNOSTIC_RELATED_SUBJECTS: usize = 8;
pub const MAXIMUM_DIAGNOSTIC_ARGUMENTS: usize = 16;
pub const MAXIMUM_DIAGNOSTIC_NOTES: usize = 8;
pub const MAXIMUM_DIAGNOSTIC_TEXT_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl From<Span> for DiagnosticSpan {
    fn from(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            line: span.line,
            column: span.column,
            end_line: span.end_line,
            end_column: span.end_column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedDiagnosticSubject {
    pub relationship: String,
    pub subject: String,
    pub span: Option<DiagnosticSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredDiagnosticV1 {
    pub schema_version: u16,
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub summary: String,
    pub source_document_id: String,
    pub content_hash: Option<String>,
    pub primary_span: Option<DiagnosticSpan>,
    pub related: Vec<RelatedDiagnosticSubject>,
    pub public_arguments: BTreeMap<String, String>,
    pub redacted_arguments: Vec<String>,
    pub notes: Vec<String>,
}

impl StructuredDiagnosticV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        summary: impl Into<String>,
        source_document_id: impl Into<String>,
        content_hash: Option<String>,
        primary_span: Option<DiagnosticSpan>,
        related: Vec<RelatedDiagnosticSubject>,
        public_arguments: BTreeMap<String, String>,
        redacted_arguments: Vec<String>,
        notes: Vec<String>,
    ) -> Result<Self, &'static str> {
        if related.len() > MAXIMUM_DIAGNOSTIC_RELATED_SUBJECTS
            || public_arguments.len() + redacted_arguments.len() > MAXIMUM_DIAGNOSTIC_ARGUMENTS
            || notes.len() > MAXIMUM_DIAGNOSTIC_NOTES
        {
            return Err("diagnostic exceeds its reviewed item bounds");
        }
        let diagnostic = Self {
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            code: code.into(),
            severity,
            summary: summary.into(),
            source_document_id: source_document_id.into(),
            content_hash,
            primary_span,
            related,
            public_arguments,
            redacted_arguments,
            notes,
        };
        diagnostic.validate()?;
        Ok(diagnostic)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != DIAGNOSTIC_SCHEMA_VERSION {
            return Err("unsupported diagnostic schema version");
        }
        if self.code.is_empty() || self.summary.is_empty() || self.source_document_id.is_empty() {
            return Err("diagnostic code, summary, and source identity are required");
        }
        if self
            .all_text()
            .any(|value| value.len() > MAXIMUM_DIAGNOSTIC_TEXT_BYTES)
        {
            return Err("diagnostic text exceeds its reviewed byte bound");
        }
        if self.related.len() > MAXIMUM_DIAGNOSTIC_RELATED_SUBJECTS
            || self.public_arguments.len() + self.redacted_arguments.len()
                > MAXIMUM_DIAGNOSTIC_ARGUMENTS
            || self.notes.len() > MAXIMUM_DIAGNOSTIC_NOTES
        {
            return Err("diagnostic exceeds its reviewed item bounds");
        }
        Ok(())
    }

    fn all_text(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.code.as_str())
            .chain(std::iter::once(self.summary.as_str()))
            .chain(std::iter::once(self.source_document_id.as_str()))
            .chain(self.content_hash.as_deref())
            .chain(
                self.related
                    .iter()
                    .flat_map(|related| [related.relationship.as_str(), related.subject.as_str()]),
            )
            .chain(
                self.public_arguments
                    .iter()
                    .flat_map(|(key, value)| [key.as_str(), value.as_str()]),
            )
            .chain(self.redacted_arguments.iter().map(String::as_str))
            .chain(self.notes.iter().map(String::as_str))
    }

    pub fn render_human(&self) -> String {
        let mut rendered = format!("{} {:?}: {}", self.code, self.severity, self.summary);
        if let Some(span) = self.primary_span {
            rendered.push_str(&format!(" at {}:{}", span.line, span.column));
        }
        rendered.push_str(&format!(" [source {}]", self.source_document_id));
        for note in &self.notes {
            rendered.push_str(&format!("\nnote: {note}"));
        }
        rendered
    }
}

pub fn source_document_identity(source: &str) -> String {
    hash_string(&format!("source-document:{source}"))
}

pub fn structured_form_diagnostic(
    source: &str,
    diagnostic: &FormDiagnostic,
) -> StructuredDiagnosticV1 {
    let (summary, truncated) = bounded_text(&diagnostic.message);
    StructuredDiagnosticV1::new(
        diagnostic.code,
        DiagnosticSeverity::Error,
        summary,
        source_document_identity(source),
        Some(hash_string(source)),
        Some(diagnostic.span.into()),
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
        if truncated {
            vec!["summary was truncated at the public diagnostic byte bound".into()]
        } else {
            Vec::new()
        },
    )
    .expect("one form diagnostic is within the fixed schema bounds")
}

fn bounded_text(value: &str) -> (String, bool) {
    if value.len() <= MAXIMUM_DIAGNOSTIC_TEXT_BYTES {
        return (value.to_string(), false);
    }
    let mut end = MAXIMUM_DIAGNOSTIC_TEXT_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parse_document, ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog,
    };
    use conduit_core::{kind_id, ConfigurationValue, KindContractRevision};

    #[test]
    fn malformed_form_human_and_json_share_exact_owned_fact() {
        let source = "not a form\n";
        let document = parse_document(source, &ProfileCatalog::new());
        let diagnostic = structured_form_diagnostic(source, &document.diagnostics[0]);
        let json = serde_json::to_value(&diagnostic).unwrap();

        assert_eq!(diagnostic.code, "CND-FRM-001");
        assert_eq!(json["schema_version"], DIAGNOSTIC_SCHEMA_VERSION);
        assert_eq!(json["source_document_id"], source_document_identity(source));
        assert_eq!(json["primary_span"]["start"], 0);
        assert!(diagnostic.render_human().contains(&diagnostic.code));
        assert!(diagnostic.render_human().contains(&diagnostic.summary));
    }

    #[test]
    fn unsupported_kind_and_configuration_keep_stable_codes_and_spans() {
        let unsupported = "form 0\n\ndemo {\n  x: missing/kind\n}\n";
        let unsupported_document = parse_document(unsupported, &ProfileCatalog::new());
        let unsupported_diagnostic =
            structured_form_diagnostic(unsupported, &unsupported_document.diagnostics[0]);
        assert_eq!(unsupported_diagnostic.code, "CND-FRM-009");
        assert!(unsupported_diagnostic.primary_span.is_some());

        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: kind_id("test/source"),
                kind_contract_revision: KindContractRevision::from("test/source@1"),
                inputs: Vec::new(),
                outputs: Vec::new(),
                configuration: vec![ConfigurationField {
                    key: "count".into(),
                    default_value: ConfigurationValue::U64(1),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: 4,
                    },
                }],
            })
            .unwrap();
        let malformed_configuration =
            "form 0\n\ndemo {\n source: test/source\n source.count = 5\n}\n";
        let configuration_document = parse_document(malformed_configuration, &catalog);
        let configuration_diagnostic = structured_form_diagnostic(
            malformed_configuration,
            &configuration_document.diagnostics[0],
        );
        assert_eq!(configuration_diagnostic.code, "CND-FRM-010");
        assert!(configuration_diagnostic.primary_span.is_some());

        let port_source = "form 0\n\ndemo {\n source: test/source\n source.missing -> sink.in\n}\n";
        let port_error = FormDiagnostic {
            code: "CND-FRM-011",
            span: Span {
                start: 43,
                end: 57,
                line: 5,
                column: 2,
                end_line: 5,
                end_column: 16,
            },
            message: "connection names an unsupported port".into(),
        };
        let port_diagnostic = structured_form_diagnostic(port_source, &port_error);
        assert_eq!(port_diagnostic.code, "CND-FRM-011");
        assert_eq!(port_diagnostic.primary_span, Some(port_error.span.into()));
    }

    #[test]
    fn unknown_schema_and_unbounded_related_subjects_fail_closed() {
        let mut diagnostic = StructuredDiagnosticV1::new(
            "CND-TEST-001",
            DiagnosticSeverity::Error,
            "test",
            "source",
            None,
            None,
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        diagnostic.schema_version = 2;
        assert_eq!(
            diagnostic.validate(),
            Err("unsupported diagnostic schema version")
        );
    }

    #[test]
    fn attacker_controlled_message_is_truncated_on_a_utf8_boundary() {
        let message = "é".repeat(MAXIMUM_DIAGNOSTIC_TEXT_BYTES);
        let source = "invalid";
        let structured = structured_form_diagnostic(
            source,
            &FormDiagnostic {
                code: "CND-FRM-TEST",
                span: Span {
                    start: 0,
                    end: source.len(),
                    line: 1,
                    column: 1,
                    end_line: 1,
                    end_column: source.len() + 1,
                },
                message,
            },
        );
        assert!(structured.summary.len() <= MAXIMUM_DIAGNOSTIC_TEXT_BYTES);
        assert_eq!(structured.notes.len(), 1);
        assert!(structured.validate().is_ok());
    }
}
