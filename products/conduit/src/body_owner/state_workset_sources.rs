//! Complete checked Source custody in the existing recoverable owner journal.
//! Storage and compiler checks confer no adoption or execution authority.
use super::*;
use conduit_body::{BodyWorkset, ResidentPlot};
use conduit_plot::{canonical_source_document_id, ExpandedAuthoringPlot};

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
    fn expand(
        &self,
        biography: &BodyBiographyEvidence,
    ) -> Result<Vec<ExpandedAuthoringPlot>, String> {
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
        let plots = self
            .entries
            .iter()
            .map(|item| {
                crate::plot_source::parse(&item.source)?.expand_named_for_authoring(&item.entry)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let checked = BodyWorkset::from_plots(plots.iter().map(|plot| {
            ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
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
            Self::Complete(_) => None,
        }
    }
    pub(super) fn workset(&self) -> Option<&WorksetSources> {
        match self {
            Self::Single(_) => None,
            Self::Complete(sources) => Some(sources),
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
) -> Result<Option<Vec<ExpandedAuthoringPlot>>, String> {
    retained(root)?
        .map(|sources| sources.expand(biography))
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
    let head = session
        .pending_archives()
        .last()
        .map(|segment| segment.digest);
    retain_inner(
        root,
        session.evidence(),
        last_execution,
        admissions.into(),
        SourceUpdate::Complete(sources),
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
