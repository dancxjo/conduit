use crate::prelude::*;
use crate::{hash_string, Span};
use alloc::collections::BTreeMap;
use serde::{Deserialize, Serialize};

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
        core::iter::once(self.code.as_str())
            .chain(core::iter::once(self.summary.as_str()))
            .chain(core::iter::once(self.source_document_id.as_str()))
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

#[cfg(test)]
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
        let (bounded, truncated) = bounded_text(&message);
        assert!(bounded.len() <= MAXIMUM_DIAGNOSTIC_TEXT_BYTES);
        assert!(bounded.is_char_boundary(bounded.len()));
        assert!(truncated);
    }
}
