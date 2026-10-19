//! Explicit, finite preparation reuse. No Plan, Host, authority or runtime cache.
use crate::{
    check_syntax_document, expand_canonical_plot_for_authoring_with_backs, parse_syntax_document,
    CanonicalBackCatalog, CanonicalExpansionDiagnostic, CheckedSyntaxDocument,
    ExpandedAuthoringPlot, ProfileCatalog, StartupCatalog, SyntaxCheckDiagnostic,
};
use alloc::{string::String, sync::Arc, vec::Vec};
use sha2::{Digest, Sha256};

/// Bounds retained entries, not admission of authored Source or catalogue data.
/// Catalogue snapshots remain caller-admitted finite preparation inputs. This
/// cache does not claim a byte-accurate total memory reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationCacheLimits {
    pub sources: usize,
    pub expansions_per_source: usize,
    pub source_bytes: usize,
}
impl Default for PreparationCacheLimits {
    fn default() -> Self {
        Self {
            sources: 4,
            expansions_per_source: 4,
            source_bytes: 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationCacheLimitError {
    Sources,
    Expansions,
    SourceBytes,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparationCacheStats {
    pub checks: u64,
    pub check_hits: u64,
    pub expansions: u64,
    pub expansion_hits: u64,
    pub source_evictions: u64,
    pub expansion_evictions: u64,
    pub unretained_checks: u64,
}
/// Immutable checked ownership. Call `check` with the current Source/catalogue
/// to establish freshness; an older token continues to name its older context.
#[derive(Clone, Debug)]
pub struct PreparedSource {
    digest: [u8; 32],
    source: Arc<str>,
    startup: Arc<StartupCatalog>,
    checked: Arc<CheckedSyntaxDocument>,
}
impl PreparedSource {
    pub fn source_digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn checked(&self) -> &Arc<CheckedSyntaxDocument> {
        &self.checked
    }
}
struct Expansion {
    entry: String,
    profiles: Arc<ProfileCatalog>,
    backs: Arc<CanonicalBackCatalog>,
    value: Arc<ExpandedAuthoringPlot>,
}
struct SourceEntry {
    prepared: PreparedSource,
    expansions: Vec<Expansion>,
}
/// Owned FIFO cache for ordinary canonical checking and authoring expansion.
/// Source SHA-256 selects candidates; exact Source bytes and complete checking
/// catalogue equality fence collisions and stale dependencies. Expansion also
/// requires exact profile and Back catalogues, including laws, private Type
/// contexts and retained Back owners. No caller-supplied cache key is trusted.
///
/// Dependency snapshots are deliberately whole catalogues. Unrelated catalogue
/// edits can miss; this API does not claim minimal subgraph invalidation.
/// Errors are never cached. Oversized Source and zero limits use the ordinary
/// cold path. Eviction drops only this cache's references; borrowed owners live.
pub struct SourcePreparationCache {
    limits: PreparationCacheLimits,
    entries: Vec<SourceEntry>,
    stats: PreparationCacheStats,
}
impl SourcePreparationCache {
    pub fn new(limits: PreparationCacheLimits) -> Result<Self, PreparationCacheLimitError> {
        if limits.sources > 16 {
            return Err(PreparationCacheLimitError::Sources);
        }
        if limits.expansions_per_source > 16 {
            return Err(PreparationCacheLimitError::Expansions);
        }
        if limits.source_bytes > 4 * 1024 * 1024 {
            return Err(PreparationCacheLimitError::SourceBytes);
        }
        Ok(Self {
            limits,
            entries: Vec::new(),
            stats: PreparationCacheStats::default(),
        })
    }
    pub fn stats(&self) -> PreparationCacheStats {
        self.stats
    }
    pub fn retained_sources(&self) -> usize {
        self.entries.len()
    }
    pub fn retained_expansions(&self) -> usize {
        self.entries.iter().map(|e| e.expansions.len()).sum()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn check(
        &mut self,
        source: &str,
        startup: &Arc<StartupCatalog>,
    ) -> Result<PreparedSource, SyntaxCheckDiagnostic> {
        let digest: [u8; 32] = Sha256::digest(source.as_bytes()).into();
        if let Some(entry) = self.entries.iter().find(|e| {
            e.prepared.digest == digest
                && e.prepared.source.as_ref() == source
                && e.prepared.startup == *startup
        }) {
            self.stats.check_hits = self.stats.check_hits.saturating_add(1);
            return Ok(entry.prepared.clone());
        }
        self.stats.checks = self.stats.checks.saturating_add(1);
        let checked = Arc::new(check_syntax_document(
            &parse_syntax_document(source),
            startup,
        )?);
        let prepared = PreparedSource {
            digest,
            source: Arc::from(source),
            startup: startup.clone(),
            checked,
        };
        if self.limits.sources == 0 || source.len() > self.limits.source_bytes {
            self.stats.unretained_checks = self.stats.unretained_checks.saturating_add(1);
        } else {
            if self.entries.len() == self.limits.sources {
                self.entries.remove(0);
                self.stats.source_evictions = self.stats.source_evictions.saturating_add(1);
            }
            self.entries.push(SourceEntry {
                prepared: prepared.clone(),
                expansions: Vec::new(),
            });
        }
        Ok(prepared)
    }
    pub fn expand(
        &mut self,
        prepared: &PreparedSource,
        entry: &str,
        profiles: &Arc<ProfileCatalog>,
        backs: &Arc<CanonicalBackCatalog>,
    ) -> Result<Arc<ExpandedAuthoringPlot>, CanonicalExpansionDiagnostic> {
        let source = self
            .entries
            .iter()
            .position(|e| Arc::ptr_eq(&e.prepared.checked, &prepared.checked));
        if let Some(cached) = source.and_then(|i| {
            self.entries[i]
                .expansions
                .iter()
                .find(|e| e.entry == entry && e.profiles == *profiles && e.backs == *backs)
        }) {
            self.stats.expansion_hits = self.stats.expansion_hits.saturating_add(1);
            return Ok(cached.value.clone());
        }
        self.stats.expansions = self.stats.expansions.saturating_add(1);
        let value = Arc::new(expand_canonical_plot_for_authoring_with_backs(
            &prepared.checked,
            entry,
            profiles,
            backs,
        )?);
        if let Some(i) = source.filter(|_| self.limits.expansions_per_source > 0) {
            let expansions = &mut self.entries[i].expansions;
            if expansions.len() == self.limits.expansions_per_source {
                expansions.remove(0);
                self.stats.expansion_evictions = self.stats.expansion_evictions.saturating_add(1);
            }
            expansions.push(Expansion {
                entry: entry.into(),
                profiles: profiles.clone(),
                backs: backs.clone(),
                value: value.clone(),
            });
        }
        Ok(value)
    }
}
