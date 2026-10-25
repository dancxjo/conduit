//! Descriptive Source checking, with compiler custody retained privately.
//! This surface cannot produce a Plan, Play, Host call or executable checked value.
mod contract;
use crate::{
    parse_syntax_document, source_span, CanonicalBackCatalog, DiagnosticSeverity,
    PreparationCacheLimits, ProfileCatalog, SourcePreparationCache, StartupCatalog,
    StructuredDiagnosticV1,
};
use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};
pub use contract::*;
use sha2::{Digest, Sha256};

/// An owner-admitted immutable reference context. Revision names are descriptive
/// owner provenance, not authenticated portable claims. Exact catalogues remain
/// here and are never reconstructed from those names or from projected Info.
pub struct SourceCheckContext {
    revision: CheckContextRevision,
    startup: Arc<StartupCatalog>,
    profiles: Arc<ProfileCatalog>,
    backs: Arc<CanonicalBackCatalog>,
}
impl SourceCheckContext {
    pub fn new(
        revision: CheckContextRevision,
        startup: StartupCatalog,
        profiles: ProfileCatalog,
        backs: CanonicalBackCatalog,
    ) -> Result<Self, SourceCheckRefusal> {
        revision.validate()?;
        Ok(Self {
            revision,
            startup: Arc::new(startup),
            profiles: Arc::new(profiles),
            backs: Arc::new(backs),
        })
    }
}

/// One immutable Source generation and exact compiler context. Construction is
/// trusted reference-owner preparation, not an untrusted context importer.
/// Checking mutates only a bounded preparation cache; all semantic inputs are
/// private immutable snapshots. Use a new owner for a new Source/context.
pub struct SourceCheckProjection {
    source: Arc<str>,
    snapshot: SourceSnapshotRevision,
    context: SourceCheckContext,
    cache: SourcePreparationCache,
}
impl SourceCheckProjection {
    pub fn new(
        owner: &str,
        generation: u64,
        source: &str,
        context: SourceCheckContext,
    ) -> Result<Self, SourceCheckRefusal> {
        if owner.is_empty() || owner.len() > MAX_CHECK_LABEL_BYTES || generation == 0 {
            return Err(SourceCheckRefusal::InvalidSnapshot);
        }
        if source.len() > crate::MAXIMUM_PLOT_SOURCE_BYTES {
            return Err(SourceCheckRefusal::SourceBytes);
        }
        Ok(Self {
            source: Arc::from(source),
            snapshot: SourceSnapshotRevision {
                owner: owner.into(),
                generation,
                sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
                source_document_id: parse_syntax_document(source)
                    .source_document_id()
                    .as_str()
                    .into(),
            },
            context,
            cache: SourcePreparationCache::new(PreparationCacheLimits::default())
                .map_err(|_| SourceCheckRefusal::Limits)?,
        })
    }
    pub fn snapshot(&self) -> &SourceSnapshotRevision {
        &self.snapshot
    }
    pub fn context(&self) -> &CheckContextRevision {
        &self.context.revision
    }

    pub fn project(
        &mut self,
        request: &SourceCheckRequest,
    ) -> Result<SourceCheckInfoV1, SourceCheckRefusal> {
        if request.snapshot != self.snapshot {
            return Err(SourceCheckRefusal::StaleSource);
        }
        request.context.validate()?;
        if request.context != self.context.revision {
            return Err(SourceCheckRefusal::UnknownContext);
        }
        if request.entry.is_empty() || request.entry.len() > MAX_CHECK_LABEL_BYTES {
            return Err(SourceCheckRefusal::EntryBytes);
        }
        request.limits.validate()?;
        let mut info = SourceCheckInfoV1 {
            schema_version: 1,
            snapshot: self.snapshot.clone(),
            context: self.context.revision.clone(),
            requested_entry: request.entry.clone(),
            limits: request.limits,
            check: CheckOutcome::Rejected,
            candidate: CandidateIdentity::default(),
            diagnostics: Vec::new(),
            dependencies: DependencyProvenance::WholeContext,
            plan: RealizationPresence::Absent,
            play: RealizationPresence::Absent,
        };
        let prepared = match self.cache.check(&self.source, &self.context.startup) {
            Ok(value) => value,
            Err(error) => {
                let span = source_span(&self.source, error.span.start..error.span.end)
                    .ok_or(SourceCheckRefusal::InvalidDiagnosticSpan)?;
                info.diagnostics.push(diagnostic(
                    &self.snapshot,
                    error.code,
                    error.message,
                    Some(span),
                )?);
                info.validate_bounds()?;
                return Ok(info);
            }
        };
        info.check = CheckOutcome::Accepted;
        info.candidate.checked_plot_id = prepared
            .checked()
            .plots
            .iter()
            .find(|plot| plot.name == request.entry)
            .map(|plot| plot.checked_plot_id.as_str().into());
        if let Some(reads) = prepared.checking_dependencies() {
            info.dependencies = DependencyProvenance::Observed {
                checking: reads.iter().map(DependencyRead::from).collect(),
                expansion: Vec::new(),
            };
        }
        match self.cache.expand(
            &prepared,
            &request.entry,
            &self.context.profiles,
            &self.context.backs,
        ) {
            Ok(expanded) => {
                info.candidate.expanded_plot_id =
                    Some(expanded.expanded.expanded_plot_id.as_str().into());
                match (
                    &mut info.dependencies,
                    self.cache.expansion_dependencies(&expanded),
                ) {
                    (DependencyProvenance::Observed { expansion, .. }, Some(reads)) => {
                        *expansion = reads.iter().map(DependencyRead::from).collect();
                    }
                    _ => info.dependencies = DependencyProvenance::WholeContext,
                }
            }
            Err(error) => {
                // Failed expansions do not publish a finished cache read set.
                // Retain whole-context provenance, never an empty read claim.
                info.dependencies = DependencyProvenance::WholeContext;
                // Expansion diagnostics carry no owned Source span. Do not invent one.
                info.diagnostics
                    .push(diagnostic(&self.snapshot, error.code, error.message, None)?);
            }
        }
        info.validate_bounds()?;
        Ok(info)
    }
}
fn diagnostic(
    snapshot: &SourceSnapshotRevision,
    code: &str,
    message: String,
    span: Option<crate::Span>,
) -> Result<SourceCheckDiagnostic, SourceCheckRefusal> {
    let diagnostic = StructuredDiagnosticV1::new(
        code,
        DiagnosticSeverity::Error,
        message,
        &snapshot.source_document_id,
        Some(snapshot.sha256.clone()),
        span.map(Into::into),
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|_| SourceCheckRefusal::DiagnosticOverflow)?;
    Ok(SourceCheckDiagnostic {
        code: diagnostic.code,
        summary: diagnostic.summary,
        source_document_id: diagnostic.source_document_id,
        content_hash: snapshot.sha256.clone(),
        primary_span: diagnostic.primary_span,
    })
}
