//! Conditional workset intent from observed facts, never installation or authority.
use conduit_body::{Body, BodyId, BodyWorkset, BodyWorksetError, ResidentPlot};
use conduit_plot::{CheckContextRevision, CheckOutcome, SourceCheckInfoV1, SourceSnapshotRevision};
use serde::Serialize;
mod native;
pub use native::propose_admitted_workset;

/// Correlates descriptive check Info with independently selected Source/context
/// revisions. Public Info remains forgeable; this is not compiler custody.
pub struct CheckedWorksetCandidate<'a> {
    pub snapshot: &'a SourceSnapshotRevision,
    pub context: &'a CheckContextRevision,
    pub report: &'a SourceCheckInfoV1,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorksetCandidateSource {
    pub snapshot: SourceSnapshotRevision,
    pub context: CheckContextRevision,
    pub entry: String,
    pub plot: ResidentPlot,
}
/// A complete desired set with compare-and-swap preconditions, not a successor
/// Body. A later authorized owner must recheck actual current state and Source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyWorksetProposalInfoV1 {
    pub schema_version: u16,
    pub expected_body_id: BodyId,
    pub expected_workload_revision: u64,
    pub proposed_workset: BodyWorkset,
    pub candidate_source: Option<WorksetCandidateSource>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorksetProposalRefusal {
    WrongBody,
    StaleWorkset,
    StaleSource,
    StaleContext,
    InvalidCheckInfo,
    UncheckedPlot,
    NoChange,
    NativeOwner,
    NativeSchema,
    NativeSourceBasis,
    Workset(BodyWorksetError),
    Encoding,
    Bytes,
}
pub const MAX_WORKSET_PROPOSAL_BYTES: usize = 32 * 1024;

/// Pure formation permits a complete revision containing retained Plots and at
/// most one new checked Source candidate. Removal needs no new Source. The
/// observed Body may be historical or Playing: this only forms conditional Info.
/// No lifecycle session, catalog, grant, Host, Plan or Play is mutated.
pub fn propose_checked_workset(
    observed: &Body,
    expected_body: &BodyId,
    expected_revision: u64,
    plots: Vec<ResidentPlot>,
    candidate: Option<CheckedWorksetCandidate<'_>>,
) -> Result<BodyWorksetProposalInfoV1, WorksetProposalRefusal> {
    use WorksetProposalRefusal as E;
    if observed.body_id != *expected_body {
        return Err(E::WrongBody);
    }
    if observed.workload_revision != expected_revision {
        return Err(E::StaleWorkset);
    }
    observed.workset.validate().map_err(E::Workset)?;
    // Canon owns ordering, duplicate, item and identity-byte laws.
    let proposed = BodyWorkset::from_plots(plots).map_err(E::Workset)?;
    let basis = candidate.map(check_candidate).transpose()?;
    for plot in proposed.plots() {
        if !observed.workset.contains(plot)
            && basis.as_ref().is_none_or(|source| source.plot != *plot)
        {
            return Err(E::UncheckedPlot);
        }
    }
    if basis
        .as_ref()
        .is_some_and(|source| !proposed.contains(&source.plot))
    {
        return Err(E::UncheckedPlot);
    }
    if proposed == observed.workset {
        return Err(E::NoChange);
    }
    let proposal = BodyWorksetProposalInfoV1 {
        schema_version: 1,
        expected_body_id: expected_body.clone(),
        expected_workload_revision: expected_revision,
        proposed_workset: proposed,
        candidate_source: basis,
    };
    if serde_json::to_vec(&proposal)
        .map_err(|_| E::Encoding)?
        .len()
        > MAX_WORKSET_PROPOSAL_BYTES
    {
        return Err(E::Bytes);
    }
    Ok(proposal)
}
fn check_candidate(
    candidate: CheckedWorksetCandidate<'_>,
) -> Result<WorksetCandidateSource, WorksetProposalRefusal> {
    use WorksetProposalRefusal as E;
    let report = candidate.report;
    if report.snapshot != *candidate.snapshot {
        return Err(E::StaleSource);
    }
    if report.context != *candidate.context {
        return Err(E::StaleContext);
    }
    let label = |s: &str| !s.is_empty() && s.len() <= conduit_plot::MAX_CHECK_LABEL_BYTES;
    let digest = |s: &str| !s.is_empty() && s.len() <= 64;
    let checked = report
        .candidate
        .checked_plot_id
        .as_deref()
        .ok_or(E::InvalidCheckInfo)?;
    if report.schema_version != 1
        || report.check != CheckOutcome::Accepted
        || report.snapshot.generation == 0
        || !label(&report.snapshot.owner)
        || report.snapshot.sha256.len() != 64
        || !report
            .snapshot
            .sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || !digest(&report.snapshot.source_document_id)
        || !digest(checked)
        || !label(&report.requested_entry)
        || report.context.grammar_edition != "conduitese-v1"
        || [
            &report.context.catalog,
            &report.context.kinds_fores,
            &report.context.types_laws,
            &report.context.backs,
        ]
        .iter()
        .any(|s| !label(s))
    {
        return Err(E::InvalidCheckInfo);
    }
    Ok(WorksetCandidateSource {
        snapshot: report.snapshot.clone(),
        context: report.context.clone(),
        entry: report.requested_entry.clone(),
        plot: ResidentPlot::new(
            report.snapshot.source_document_id.as_str().into(),
            checked.into(),
        ),
    })
}
