use crate::{CatalogueRead, CatalogueTable, DiagnosticSpan};
use alloc::{string::String, vec::Vec};
use serde::Serialize;
pub const MAX_CHECK_LABEL_BYTES: usize = 256;
pub const MAX_CHECK_DIAGNOSTICS: usize = 16;
pub const MAX_CHECK_DEPENDENCIES: usize = 1024;
pub const MAX_CHECK_DIAGNOSTIC_BYTES: usize = 16 * 1024;
pub const MAX_CHECK_DEPENDENCY_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceSnapshotRevision {
    pub owner: String,
    pub generation: u64,
    pub sha256: String,
    pub source_document_id: String,
}
/// Names refer to the owner's exact immutable catalogues, never to ambient
/// defaults. All five revisions are required, even for empty input sets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CheckContextRevision {
    pub grammar_edition: String,
    pub catalog: String,
    pub kinds_fores: String,
    pub types_laws: String,
    pub backs: String,
}
impl CheckContextRevision {
    pub(super) fn validate(&self) -> Result<(), SourceCheckRefusal> {
        if self.grammar_edition != "conduitese-v1" {
            return Err(SourceCheckRefusal::UnknownGrammar);
        }
        if [
            &self.catalog,
            &self.kinds_fores,
            &self.types_laws,
            &self.backs,
        ]
        .iter()
        .any(|value| value.is_empty() || value.len() > MAX_CHECK_LABEL_BYTES)
        {
            return Err(SourceCheckRefusal::UnknownContext);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SourceCheckLimits {
    pub diagnostics: usize,
    pub diagnostic_bytes: usize,
    pub dependencies: usize,
    pub dependency_bytes: usize,
}
impl Default for SourceCheckLimits {
    fn default() -> Self {
        Self {
            diagnostics: MAX_CHECK_DIAGNOSTICS,
            diagnostic_bytes: MAX_CHECK_DIAGNOSTIC_BYTES,
            dependencies: MAX_CHECK_DEPENDENCIES,
            dependency_bytes: MAX_CHECK_DEPENDENCY_BYTES,
        }
    }
}
impl SourceCheckLimits {
    pub(super) fn validate(self) -> Result<(), SourceCheckRefusal> {
        if self.diagnostics > MAX_CHECK_DIAGNOSTICS
            || self.diagnostic_bytes > MAX_CHECK_DIAGNOSTIC_BYTES
            || self.dependencies > MAX_CHECK_DEPENDENCIES
            || self.dependency_bytes > MAX_CHECK_DEPENDENCY_BYTES
        {
            return Err(SourceCheckRefusal::Limits);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCheckRequest {
    pub snapshot: SourceSnapshotRevision,
    pub context: CheckContextRevision,
    pub entry: String,
    pub limits: SourceCheckLimits,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CandidateIdentity {
    pub checked_plot_id: Option<String>,
    pub expanded_plot_id: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckOutcome {
    Accepted,
    Rejected,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RealizationPresence {
    Absent,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DependencyRead {
    pub table: String,
    pub key: String,
}
impl From<&CatalogueRead> for DependencyRead {
    fn from(value: &CatalogueRead) -> Self {
        Self {
            table: match value.table {
                CatalogueTable::Signature => "signature",
                CatalogueTable::Fore => "fore",
                CatalogueTable::VariadicFore => "variadic-fore",
                CatalogueTable::Projection => "projection",
                CatalogueTable::CanonicalKind => "canonical-kind",
                CatalogueTable::VariadicKind => "variadic-kind",
            }
            .into(),
            key: value.key.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DependencyProvenance {
    Observed {
        checking: Vec<DependencyRead>,
        expansion: Vec<DependencyRead>,
    },
    WholeContext,
}
/// Finite normalized error projection. The canonical structured diagnostic
/// validates code/message bounds before extraction; no compiler AST escapes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceCheckDiagnostic {
    pub code: String,
    pub summary: String,
    pub source_document_id: String,
    pub content_hash: String,
    pub primary_span: Option<DiagnosticSpan>,
}
/// Serialized Info is a description. No decoder into CheckedSyntaxDocument,
/// ExpandedAuthoringPlot, Plan, Play or a bearer capability exists here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceCheckInfoV1 {
    pub schema_version: u16,
    pub snapshot: SourceSnapshotRevision,
    pub context: CheckContextRevision,
    pub requested_entry: String,
    pub limits: SourceCheckLimits,
    pub check: CheckOutcome,
    pub candidate: CandidateIdentity,
    pub diagnostics: Vec<SourceCheckDiagnostic>,
    pub dependencies: DependencyProvenance,
    pub plan: RealizationPresence,
    pub play: RealizationPresence,
}
impl SourceCheckInfoV1 {
    pub(super) fn validate_bounds(&self) -> Result<(), SourceCheckRefusal> {
        // Existing diagnostics have finite item counts. Count every retained
        // text field conservatively, including identities and optional detail.
        let mut bytes = 0usize;
        for d in &self.diagnostics {
            bytes = bytes
                .saturating_add(d.code.len())
                .saturating_add(d.summary.len())
                .saturating_add(d.source_document_id.len())
                .saturating_add(d.content_hash.len());
        }
        if self.diagnostics.len() > self.limits.diagnostics || bytes > self.limits.diagnostic_bytes
        {
            return Err(SourceCheckRefusal::DiagnosticOverflow);
        }
        if let DependencyProvenance::Observed {
            checking,
            expansion,
        } = &self.dependencies
        {
            let bytes = checking.iter().chain(expansion).fold(0usize, |n, r| {
                n.saturating_add(r.table.len()).saturating_add(r.key.len())
            });
            if checking.len().saturating_add(expansion.len()) > self.limits.dependencies
                || bytes > self.limits.dependency_bytes
            {
                return Err(SourceCheckRefusal::DependencyOverflow);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceCheckRefusal {
    InvalidSnapshot,
    SourceBytes,
    StaleSource,
    UnknownGrammar,
    UnknownContext,
    EntryBytes,
    Limits,
    InvalidDiagnosticSpan,
    DiagnosticOverflow,
    DependencyOverflow,
}
