//! Allocator-free structured diagnostics.

/// Structured diagnostic schema version.
pub const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;

/// Stable diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

impl DiagnosticSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// Exact source extent. Byte ranges are zero-based and end-exclusive;
/// presentation locations are one-based.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticSpan<'a> {
    pub document_id: &'a str,
    pub content_hash: Option<&'a str>,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u64,
    pub column: u64,
    pub end_line: u64,
    pub end_column: u64,
}

/// One related location or semantic subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticRelated<'a> {
    pub label: &'a str,
    pub span: Option<DiagnosticSpan<'a>>,
    pub subject: Option<&'a str>,
}

/// Sensitivity-safe diagnostic argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticArgumentValue<'a> {
    Public(&'a str),
    Redacted {
        sensitivity: &'a str,
        value_type: &'a str,
        byte_len: Option<u64>,
    },
}

/// Named structured argument. Safe prose never interpolates protected bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticArgument<'a> {
    pub name: &'a str,
    pub value: DiagnosticArgumentValue<'a>,
}

/// Confidence and automation boundary for a proposed fix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixApplicability {
    MachineApplicable,
    MaybeIncorrect,
}

impl FixApplicability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MachineApplicable => "machine-applicable",
            Self::MaybeIncorrect => "maybe-incorrect",
        }
    }
}

/// One source edit guarded by the exact document content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticEdit<'a> {
    pub document_id: &'a str,
    pub precondition_hash: &'a str,
    pub byte_start: u64,
    pub byte_end: u64,
    pub replacement: &'a str,
}

/// One unapplied fix proposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticFix<'a> {
    pub id: &'a str,
    pub message: &'a str,
    pub applicability: FixApplicability,
    pub edits: &'a [DiagnosticEdit<'a>],
}

/// Complete borrowed diagnostic data. Prose is safe, concise presentation;
/// values travel only through structured public or redacted arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostic<'a> {
    pub schema_version: u32,
    pub code: &'a str,
    pub severity: DiagnosticSeverity,
    pub message: &'a str,
    pub primary: Option<DiagnosticSpan<'a>>,
    pub related: &'a [DiagnosticRelated<'a>],
    pub arguments: &'a [DiagnosticArgument<'a>],
    pub notes: &'a [&'a str],
    pub help: Option<&'a str>,
    pub fixes: &'a [DiagnosticFix<'a>],
    pub semantic_path: Option<&'a str>,
    pub causes: &'a [&'a str],
}

impl Diagnostic<'_> {
    /// Validates the portable structure without allocating.
    pub fn validate(&self) -> Result<(), DiagnosticContractError> {
        if self.schema_version != DIAGNOSTIC_SCHEMA_VERSION {
            return Err(DiagnosticContractError::UnsupportedVersion);
        }
        if !valid_code(self.code) || self.causes.iter().any(|code| !valid_code(code)) {
            return Err(DiagnosticContractError::InvalidCode);
        }
        if self.message.is_empty() {
            return Err(DiagnosticContractError::EmptyMessage);
        }
        if self.primary.is_some_and(|span| !valid_span(span))
            || self.related.iter().any(|related| {
                related.label.is_empty()
                    || related.subject.is_some_and(str::is_empty)
                    || related.span.is_some_and(|span| !valid_span(span))
            })
        {
            return Err(DiagnosticContractError::InvalidSpan);
        }
        if self.arguments.iter().any(|argument| {
            argument.name.is_empty()
                || matches!(
                    argument.value,
                    DiagnosticArgumentValue::Redacted {
                        sensitivity: "",
                        ..
                    } | DiagnosticArgumentValue::Redacted { value_type: "", .. }
                )
        }) || self.notes.iter().any(|note| note.is_empty())
            || self.help.is_some_and(str::is_empty)
        {
            return Err(DiagnosticContractError::EmptyMessage);
        }
        for fix in self.fixes {
            if fix.id.is_empty() || fix.message.is_empty() || fix.edits.is_empty() {
                return Err(DiagnosticContractError::InvalidFix);
            }
            for edit in fix.edits {
                if edit.document_id.is_empty()
                    || edit.byte_start > edit.byte_end
                    || !valid_sha256(edit.precondition_hash)
                {
                    return Err(DiagnosticContractError::InvalidFix);
                }
            }
            for (index, edit) in fix.edits.iter().enumerate() {
                if fix.edits[..index].iter().any(|prior| {
                    prior.document_id == edit.document_id
                        && ((prior.byte_start < edit.byte_end && edit.byte_start < prior.byte_end)
                            || (prior.byte_start == prior.byte_end
                                && edit.byte_start == edit.byte_end
                                && prior.byte_start == edit.byte_start))
                }) {
                    return Err(DiagnosticContractError::InvalidFix);
                }
            }
        }
        Ok(())
    }
}

fn valid_span(span: DiagnosticSpan<'_>) -> bool {
    !span.document_id.is_empty()
        && span.content_hash.is_none_or(valid_sha256)
        && span.byte_start <= span.byte_end
        && span.line > 0
        && span.column > 0
        && span.end_line > 0
        && span.end_column > 0
        && (span.line < span.end_line
            || (span.line == span.end_line && span.column <= span.end_column))
}

fn valid_code(code: &str) -> bool {
    let Some(rest) = code.strip_prefix("CND-") else {
        return false;
    };
    let Some((family, number)) = rest.rsplit_once('-') else {
        return false;
    };
    !family.is_empty()
        && family.bytes().all(|byte| byte.is_ascii_uppercase())
        && number.len() == 3
        && number.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Invalid portable diagnostic structure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticContractError {
    UnsupportedVersion,
    InvalidCode,
    EmptyMessage,
    InvalidSpan,
    InvalidFix,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn validates_borrowed_diagnostics_and_rejects_overlapping_edits() {
        let edits = [
            DiagnosticEdit {
                document_id: "mem://fixture/root.panel",
                precondition_hash: HASH,
                byte_start: 1,
                byte_end: 3,
                replacement: "a",
            },
            DiagnosticEdit {
                document_id: "mem://fixture/root.panel",
                precondition_hash: HASH,
                byte_start: 2,
                byte_end: 4,
                replacement: "b",
            },
        ];
        let fixes = [DiagnosticFix {
            id: "overlap",
            message: "overlapping edits",
            applicability: FixApplicability::MachineApplicable,
            edits: &edits,
        }];
        let diagnostic = Diagnostic {
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            code: "CND-SRC-001",
            severity: DiagnosticSeverity::Error,
            message: "source is malformed",
            primary: None,
            related: &[],
            arguments: &[],
            notes: &[],
            help: None,
            fixes: &fixes,
            semantic_path: None,
            causes: &[],
        };
        assert_eq!(
            diagnostic.validate(),
            Err(DiagnosticContractError::InvalidFix)
        );
    }
}
