//! Complete checked Source custody in the existing recoverable owner journal.
//! Storage and compiler checks confer no adoption or execution authority.
use super::super::retained_source::RetainedPlotSource;
use super::*;
use conduit_body::{BodyWorkset, ResidentPlot};
use conduit_plot::canonical_source_document_id;

const SCHEMA: &str = "conduit.body/workset-sources@1";
const FILE: &str = "body/workset-sources.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct WorksetSources {
    schema: String,
    entries: Vec<SourceEntry>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceEntry {
    source: String,
    entry: String,
}
impl WorksetSources {
    /// Root supplies captured immutable text and explicit entry names, never paths.
    #[allow(dead_code)] // The authorized replacement action is not installed yet.
    pub(in super::super) fn checked(
        biography: &BodyBiographyEvidence,
        sources: Vec<(String, String)>,
    ) -> Result<Self, String> {
        let result = Self {
            schema: SCHEMA.into(),
            entries: sources
                .into_iter()
                .map(|(source, entry)| SourceEntry { source, entry })
                .collect(),
        };
        result.expand(biography)?;
        Ok(result)
    }
    /// Descriptive acquisition checks only. No entry is parsed, checked or made
    /// executable; caller workspace and Source authority remain independent.
    fn validate_captured_bytes(&self, biography: &BodyBiographyEvidence) -> Result<(), String> {
        if self.schema != SCHEMA
            || self.entries.len() != biography.body.workset.plots().len()
            || self.entries.len() > conduit_body::MAX_BODY_PLOTS
        {
            return Err("complete workset Source count or schema differs".into());
        }
        let mut bytes = 0usize;
        // Check every byte/identity bound before allowing any parser invocation.
        for item in &self.entries {
            bytes = bytes
                .checked_add(item.source.len())
                .ok_or("workset Source size overflow")?;
            if bytes > super::super::MAXIMUM_SOURCE as usize
                || item.entry.is_empty()
                || item.entry.len() > conduit_plot::MAX_CHECK_LABEL_BYTES
            {
                return Err("complete workset Source bound exhausted".into());
            }
            let id = canonical_source_document_id(&item.source);
            if !biography
                .body
                .workset
                .plots()
                .iter()
                .any(|plot| plot.source_document_id == id)
            {
                return Err("workset Source bytes differ from retained meaning".into());
            }
        }
        Ok(())
    }
    pub(in super::super) fn expand(
        &self,
        biography: &BodyBiographyEvidence,
    ) -> Result<Vec<RetainedPlotSource>, String> {
        self.validate_captured_bytes(biography)?;
        Self::expand_entries(self.entries.iter().cloned(), biography)
    }

    /// Consume the captured inventory so compilation retains each Source once.
    pub(in super::super) fn into_expanded(
        self,
        biography: &BodyBiographyEvidence,
    ) -> Result<Vec<RetainedPlotSource>, String> {
        self.validate_captured_bytes(biography)?;
        Self::expand_entries(self.entries, biography)
    }

    fn expand_entries(
        entries: impl IntoIterator<Item = SourceEntry>,
        biography: &BodyBiographyEvidence,
    ) -> Result<Vec<RetainedPlotSource>, String> {
        let plots = entries
            .into_iter()
            .map(|item| {
                let source = crate::plot_source::parse_owned(item.source)?;
                let plot = source.expand_named_for_authoring(&item.entry)?;
                Ok(RetainedPlotSource { source, plot })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let checked = BodyWorkset::from_plots(plots.iter().map(|plot| {
            ResidentPlot::new(
                plot.plot.expanded.source_document_id.clone(),
                plot.plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(|error| format!("checked Source workset: {error:?}"))?;
        if checked != biography.body.workset {
            return Err("checked Source entries differ from complete retained workset".into());
        }
        Ok(plots)
    }
}

pub(super) enum SourceUpdate<'a> {
    Single(Option<&'a [u8]>),
    #[allow(dead_code)] // Only the future authorized owner action may select this.
    Complete(&'a WorksetSources),
    #[allow(dead_code)] // The installed owner action must supply current authority separately.
    Revision(&'a WorksetSources, &'a BodyBiographyEvidence),
}
impl<'a> From<Option<&'a [u8]>> for SourceUpdate<'a> {
    fn from(value: Option<&'a [u8]>) -> Self {
        Self::Single(value)
    }
}
impl SourceUpdate<'_> {
    pub(super) fn single(&self) -> Option<&[u8]> {
        match self {
            Self::Single(source) => *source,
            Self::Complete(_) | Self::Revision(_, _) => None,
        }
    }
    pub(super) fn workset(&self) -> Option<&WorksetSources> {
        match self {
            Self::Single(_) => None,
            Self::Complete(sources) | Self::Revision(sources, _) => Some(sources),
        }
    }
    /// Compare after journal recovery, before preparation or publication. The
    /// foreground owner already holds its exclusive Body lock; this is a CAS
    /// precondition, not a replacement for that lock or adoption authority.
    pub(super) fn validate_transition(
        &self,
        root: &Path,
        next: &BodyBiographyEvidence,
    ) -> Result<(), String> {
        match self {
            Self::Single(_) => Ok(()),
            Self::Complete(_) => {
                if let Some(current) = load(root)? {
                    if current.body.workset != next.body.workset
                        || current.body.workload_revision != next.body.workload_revision
                    {
                        return Err(
                            "complete Source replacement requires expected current revision".into(),
                        );
                    }
                }
                Ok(())
            }
            Self::Revision(_, expected) => {
                let current = load(root)?.ok_or("workset revision requires a retained Body")?;
                if &current != *expected {
                    return Err("stale owner workset revision basis".into());
                }
                if next.body_id != current.body_id
                    || next.body.state != conduit_body::BodyState::Lulled
                    || current.body.state != conduit_body::BodyState::Lulled
                    || next.body.workload_revision <= current.body.workload_revision
                    || next.body.workset == current.body.workset
                {
                    return Err(
                        "workset revision requires a new Lulled revision of the same Body".into(),
                    );
                }
                Ok(())
            }
        }
    }
    pub(super) fn validate(
        &self,
        root: &Path,
        biography: &BodyBiographyEvidence,
    ) -> Result<(), String> {
        if let Some(source) = self.single() {
            validate_source(biography, source)?;
        }
        if let Some(sources) = self.workset() {
            sources.expand(biography)?;
        } else {
            checked(root, biography)?;
        }
        Ok(())
    }
}

fn retained(root: &Path) -> Result<Option<WorksetSources>, String> {
    let path = root.join(FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = bounded_read(&path, MAXIMUM_STATE)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("retained workset Sources invalid: {error}"))
}
pub(in super::super) fn checked(
    root: &Path,
    biography: &BodyBiographyEvidence,
) -> Result<Option<Vec<RetainedPlotSource>>, String> {
    captured(root, biography)?
        .map(|sources| sources.expand(biography))
        .transpose()
}
/// Own the bounded retained inventory before invoking a compiler. These checks
/// establish byte/Source correspondence only, not checked entry identities or
/// authorization of a new Source. The caller must fund capture/JSON workspace.
pub(in super::super) fn captured(
    root: &Path,
    biography: &BodyBiographyEvidence,
) -> Result<Option<WorksetSources>, String> {
    retained(root)?
        .map(|sources| {
            sources.validate_captured_bytes(biography)?;
            Ok(sources)
        })
        .transpose()
}
pub(super) fn validate_transaction(root: &Path, transaction: &Transaction) -> Result<(), String> {
    if transaction.source.is_some() && transaction.workset_sources.is_some() {
        return Err("owner transaction has conflicting Source selections".into());
    }
    let update = match &transaction.workset_sources {
        Some(sources) => SourceUpdate::Complete(sources),
        None => SourceUpdate::Single(transaction.source.as_deref()),
    };
    update.validate(root, &transaction.biography)
}
pub(super) fn commit(root: &Path, transaction: &Transaction) -> Result<(), String> {
    if let Some(sources) = &transaction.workset_sources {
        write_json_atomic(&root.join(FILE), sources)?;
    }
    Ok(())
}

/// Uses the same biography/admission/archive transaction; no separate commit log.
#[allow(dead_code)] // Installation awaits the capability-mediated revision action.
pub(in super::super) fn retain_session(
    root: &Path,
    session: &mut BodyLifecycleSession,
    sources: &WorksetSources,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
) -> Result<(), String> {
    retain_selected_session(
        root,
        session,
        SourceUpdate::Complete(sources),
        last_execution,
        admissions,
    )
}

/// Exact-current journal transition only. Capability/Source/Plan authorization
/// remains the owner action's responsibility; no request or copied ID grants it.
#[allow(dead_code)]
pub(in super::super) fn retain_revision(
    root: &Path,
    session: &mut BodyLifecycleSession,
    expected: &BodyBiographyEvidence,
    sources: &WorksetSources,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
) -> Result<(), String> {
    if session.realization().is_some() {
        return Err("workset revision requires actual Play retirement".into());
    }
    retain_selected_session(
        root,
        session,
        SourceUpdate::Revision(sources, expected),
        last_execution,
        admissions,
    )
}

fn retain_selected_session(
    root: &Path,
    session: &mut BodyLifecycleSession,
    source: SourceUpdate<'_>,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
) -> Result<(), String> {
    let head = session
        .pending_archives()
        .last()
        .map(|segment| segment.digest);
    retain_inner(
        root,
        session.evidence(),
        last_execution,
        admissions.into(),
        source,
        None,
        session.pending_archives(),
    )?;
    if let Some(head) = head {
        session
            .acknowledge_archives(head)
            .map_err(|error| format!("acknowledge committed biography archive: {error:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "state_workset_sources_tests.rs"]
mod tests;
