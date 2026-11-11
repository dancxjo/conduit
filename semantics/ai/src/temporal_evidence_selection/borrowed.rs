//! Borrowed millisecond provenance and fixed-capacity temporal selection.
use super::{
    MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES as CAPACITY, MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES,
};
use crate::{
    ClockBasis, EntityBoundary, TemporalEvidenceCandidate, TemporalEvidenceSelectionRefusal as R,
    TemporalProvenance, TemporalReference, TemporalRetrievalIntent, TemporalSource,
    TransitionDirection,
};

/// Primitive intent view for native codecs; constructing it never allocates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedTemporalRetrievalIntent {
    EarliestEvidence,
    LatestEvidence,
    StateValidAt(u64),
    Transition(TransitionDirection),
    DurationSince(EntityBoundary),
    EventOrdering,
    EvidenceWithin { start: u64, end: u64 },
}
impl From<&TemporalRetrievalIntent> for BorrowedTemporalRetrievalIntent {
    fn from(value: &TemporalRetrievalIntent) -> Self {
        match value {
            TemporalRetrievalIntent::EarliestEvidence => Self::EarliestEvidence,
            TemporalRetrievalIntent::LatestEvidence => Self::LatestEvidence,
            TemporalRetrievalIntent::StateValidAt(v) => Self::StateValidAt(*v.instant()),
            TemporalRetrievalIntent::Transition(v) => Self::Transition(*v.direction()),
            TemporalRetrievalIntent::DurationSince(v) => Self::DurationSince(*v.boundary()),
            TemporalRetrievalIntent::EventOrdering => Self::EventOrdering,
            TemporalRetrievalIntent::EvidenceWithin(v) => Self::EvidenceWithin {
                start: *v.window().start(),
                end: *v.window().end(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedTemporalClockBasis<'a> {
    UnixEpochMilliseconds,
    MonotonicMilliseconds(&'a str),
}
impl<'a> From<&'a ClockBasis> for BorrowedTemporalClockBasis<'a> {
    fn from(value: &'a ClockBasis) -> Self {
        match value {
            ClockBasis::UnixEpochMilliseconds => Self::UnixEpochMilliseconds,
            ClockBasis::MonotonicMilliseconds(value) => {
                Self::MonotonicMilliseconds(value.identity().get().as_str())
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedTemporalReference<'a> {
    pub reference_at: u64,
    pub clock_basis: BorrowedTemporalClockBasis<'a>,
}
impl<'a> From<&'a TemporalReference> for BorrowedTemporalReference<'a> {
    fn from(value: &'a TemporalReference) -> Self {
        Self {
            reference_at: value.reference_at,
            clock_basis: (&value.clock_basis).into(),
        }
    }
}
/// All ticks are milliseconds. Uncertainty is retained, but selection follows
/// the existing exact source-timestamp ordering, not interval ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedTemporalProvenance<'a> {
    pub event_at: Option<u64>,
    pub valid_from: Option<u64>,
    pub valid_until: Option<u64>,
    pub observed_at: Option<u64>,
    pub recorded_at: Option<u64>,
    pub ingested_at: Option<u64>,
    pub retrieved_at: u64,
    pub reference_at: u64,
    pub clock_basis: BorrowedTemporalClockBasis<'a>,
    pub uncertainty_millis: Option<u64>,
}
impl<'a> From<&'a TemporalProvenance> for BorrowedTemporalProvenance<'a> {
    fn from(v: &'a TemporalProvenance) -> Self {
        Self {
            event_at: v.event_at,
            valid_from: v.valid_from,
            valid_until: v.valid_until,
            observed_at: v.observed_at,
            recorded_at: v.recorded_at,
            ingested_at: v.ingested_at,
            retrieved_at: v.retrieved_at,
            reference_at: v.reference_at,
            clock_basis: (&v.clock_basis).into(),
            uncertainty_millis: v.uncertainty_millis,
        }
    }
}
impl BorrowedTemporalProvenance<'_> {
    fn source_instant(self, source: TemporalSource) -> Result<u64, R> {
        match source {
            TemporalSource::Event => self.event_at,
            TemporalSource::ValidFrom => self.valid_from,
            TemporalSource::ValidUntil => self.valid_until,
            TemporalSource::Observed => self.observed_at,
            TemporalSource::Recorded => self.recorded_at,
            TemporalSource::Ingested => self.ingested_at,
            TemporalSource::Retrieved => Some(self.retrieved_at),
        }
        .ok_or(R::MissingSourceTime)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedTemporalEvidenceCandidate<'a> {
    pub identity: &'a str,
    pub provenance: BorrowedTemporalProvenance<'a>,
    pub source: TemporalSource,
    pub boundary: Option<EntityBoundary>,
    pub transition: Option<TransitionDirection>,
}
impl<'a> From<&'a TemporalEvidenceCandidate> for BorrowedTemporalEvidenceCandidate<'a> {
    fn from(v: &'a TemporalEvidenceCandidate) -> Self {
        Self {
            identity: v.identity.get().as_str(),
            provenance: (&v.provenance).into(),
            source: v.source,
            boundary: v.boundary,
            transition: v.transition,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalEvidenceSelectionStatus {
    Selected,
    NoHistory,
    NeedEarlierHistory,
    BoundaryUnavailable,
}
/// Caller-owned fixed storage. A refusal clears any previous result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalEvidenceIndexSelection {
    indices: [usize; CAPACITY],
    len: usize,
    status: TemporalEvidenceSelectionStatus,
}
impl Default for TemporalEvidenceIndexSelection {
    fn default() -> Self {
        Self::new()
    }
}
impl TemporalEvidenceIndexSelection {
    pub const fn new() -> Self {
        Self {
            indices: [0; CAPACITY],
            len: 0,
            status: TemporalEvidenceSelectionStatus::NoHistory,
        }
    }
    pub fn indices(&self) -> &[usize] {
        &self.indices[..self.len]
    }
    pub const fn status(&self) -> TemporalEvidenceSelectionStatus {
        self.status
    }
}
/// Clock labels are compared exactly; as with TemporalReference::validate, this
/// selector does not validate their syntax. Native adapters validate source Types.
/// Select from at most 128 borrowed candidates without allocation. Empty input
/// is explicitly NoHistory; it does not claim that earlier history is absent.
pub fn select_temporal_evidence_into(
    reference: BorrowedTemporalReference<'_>,
    candidates: &[BorrowedTemporalEvidenceCandidate<'_>],
    intent: &TemporalRetrievalIntent,
    earliest_history_complete: bool,
    output: &mut TemporalEvidenceIndexSelection,
) -> Result<(), R> {
    select_borrowed_temporal_evidence_into(
        reference,
        candidates,
        intent.into(),
        earliest_history_complete,
        output,
    )
}
/// The fully borrowed selector, including primitive intent. Reversed query
/// windows refuse before empty-history handling, matching owned validation.
pub fn select_borrowed_temporal_evidence_into(
    reference: BorrowedTemporalReference<'_>,
    candidates: &[BorrowedTemporalEvidenceCandidate<'_>],
    intent: BorrowedTemporalRetrievalIntent,
    earliest_history_complete: bool,
    output: &mut TemporalEvidenceIndexSelection,
) -> Result<(), R> {
    select_indices(
        reference,
        candidates.len(),
        |i| candidates[i],
        intent,
        earliest_history_complete,
        true,
        output,
    )
}
pub(super) fn select_indices<'a>(
    reference: BorrowedTemporalReference<'_>,
    count: usize,
    candidate: impl Fn(usize) -> BorrowedTemporalEvidenceCandidate<'a>,
    intent: BorrowedTemporalRetrievalIntent,
    complete: bool,
    allow_empty: bool,
    output: &mut TemporalEvidenceIndexSelection,
) -> Result<(), R> {
    *output = TemporalEvidenceIndexSelection::new();
    if matches!(intent, BorrowedTemporalRetrievalIntent::EvidenceWithin {start,end} if start > end)
    {
        return Err(R::InvalidIntent);
    }
    if count == 0 {
        return if allow_empty {
            Ok(())
        } else {
            Err(R::EmptyCandidates)
        };
    }
    if count > CAPACITY {
        return Err(R::TooManyCandidates);
    }
    for i in 0..count {
        let c = candidate(i);
        if c.identity.is_empty() {
            return Err(R::EmptyIdentity);
        }
        if c.identity.len() > MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES {
            return Err(R::IdentityTooLarge);
        }
        if (i + 1..count).any(|j| candidate(j).identity == c.identity) {
            return Err(R::DuplicateIdentity);
        }
        let p = c.provenance;
        if p.valid_from.zip(p.valid_until).is_some_and(|(a, b)| a > b)
            || p.retrieved_at > p.reference_at
        {
            return Err(R::InvalidProvenance);
        }
        if p.reference_at != reference.reference_at || p.clock_basis != reference.clock_basis {
            return Err(R::ReferenceMismatch);
        }
        p.source_instant(c.source)?;
    }
    if matches!(intent, BorrowedTemporalRetrievalIntent::EarliestEvidence) && !complete {
        output.status = TemporalEvidenceSelectionStatus::NeedEarlierHistory;
        return Ok(());
    }
    let mut selected = TemporalEvidenceIndexSelection::new();
    for i in 0..count {
        let c = candidate(i);
        let instant = c.provenance.source_instant(c.source)?;
        if matches_candidate(c, instant, intent) {
            let mut position = selected.len;
            while position > 0 {
                let previous = candidate(selected.indices[position - 1]);
                let previous_instant = previous.provenance.source_instant(previous.source)?;
                if (previous_instant, previous.identity) <= (instant, c.identity) {
                    break;
                }
                selected.indices[position] = selected.indices[position - 1];
                position -= 1;
            }
            selected.indices[position] = i;
            selected.len += 1;
        }
    }
    if selected.len == 0 {
        selected.status = if needs_earlier(intent) && !complete {
            TemporalEvidenceSelectionStatus::NeedEarlierHistory
        } else {
            TemporalEvidenceSelectionStatus::BoundaryUnavailable
        };
    } else {
        selected.status = TemporalEvidenceSelectionStatus::Selected;
        match intent {
            BorrowedTemporalRetrievalIntent::EarliestEvidence
            | BorrowedTemporalRetrievalIntent::DurationSince(_) => selected.len = 1,
            BorrowedTemporalRetrievalIntent::LatestEvidence => {
                selected.indices[0] = selected.indices[selected.len - 1];
                selected.len = 1
            }
            _ => {}
        }
    }
    *output = selected;
    Ok(())
}
fn matches_candidate(
    c: BorrowedTemporalEvidenceCandidate<'_>,
    instant: u64,
    intent: BorrowedTemporalRetrievalIntent,
) -> bool {
    match intent {
        BorrowedTemporalRetrievalIntent::EarliestEvidence
        | BorrowedTemporalRetrievalIntent::LatestEvidence
        | BorrowedTemporalRetrievalIntent::EventOrdering => true,
        BorrowedTemporalRetrievalIntent::StateValidAt(v) => {
            c.provenance.valid_from.is_some_and(|start| start <= v)
                && c.provenance.valid_until.is_none_or(|end| v <= end)
        }
        BorrowedTemporalRetrievalIntent::Transition(v) => c.transition == Some(v),
        BorrowedTemporalRetrievalIntent::DurationSince(v) => c.boundary == Some(v),
        BorrowedTemporalRetrievalIntent::EvidenceWithin { start, end } => {
            start <= instant && instant <= end
        }
    }
}
fn needs_earlier(intent: BorrowedTemporalRetrievalIntent) -> bool {
    matches!(
        intent,
        BorrowedTemporalRetrievalIntent::EarliestEvidence
            | BorrowedTemporalRetrievalIntent::StateValidAt(_)
            | BorrowedTemporalRetrievalIntent::Transition(_)
            | BorrowedTemporalRetrievalIntent::DurationSince(_)
            | BorrowedTemporalRetrievalIntent::EvidenceWithin { .. }
    )
}
