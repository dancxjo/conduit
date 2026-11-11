//! Explicit temporal windows over the immutable pre-selection episode snapshot.
//! Missing times are reported as omissions; episode counters never become time.
use super::episode_retrieval_evidence::{
    EpisodeRetrievalEvidenceError, PreparedEpisodeRetrievalEvidence,
};
use super::episode_retrieval_snapshot::{
    BorrowedEpisodeRetrievalSnapshot, EpisodeRetrievalSnapshotRefusal,
};
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, TemporalRelation, TemporalScale,
    ValidatedCanonicalStructuredValue,
};
#[path = "episode_retrieval_window/encoding.rs"]
mod encoding;
use encoding::*;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
const MAX: usize = conduit_ai::TEMPORAL_EVIDENCE_FLOW_MAX_BYTES;
pub const MAXIMUM_EPISODE_TEMPORAL_CANDIDATES: usize =
    4 * super::episode_retrieval_evidence::MAXIMUM_EPISODE_EVIDENCE_ITEMS;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EpisodeTemporalOmissionReason {
    MissingEvidence,
    MissingObservedTime,
    UnsupportedTime,
    ClockMismatch,
    InvalidInterval,
    FutureSource,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EpisodeTemporalOmission {
    pub episode: u64,
    pub item: Option<u16>,
    pub reason: EpisodeTemporalOmissionReason,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct EpisodeRetrievalCandidateKey {
    pub episode: u64,
    pub item: u16,
}
impl EpisodeRetrievalCandidateKey {
    /// Write this window-local candidate name without constructing owned text.
    pub fn write_identity<'a>(&self, output: &'a mut [u8; 64]) -> &'a str {
        let (bytes, length) = encoding::candidate_identity(*self);
        *output = bytes;
        core::str::from_utf8(&output[..length]).expect("ASCII candidate identity")
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EpisodeRetrievalResolution {
    Selected,
    NoHistory,
    NeedEarlierHistory,
    BoundaryUnavailable,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EpisodeRetrievalWindowRefusal {
    Snapshot(EpisodeRetrievalSnapshotRefusal),
    Evidence(EpisodeRetrievalEvidenceError),
    Malformed,
    ByteBound,
    ItemBound,
    NoWindow,
    SnapshotMismatch,
    UnknownSelection,
    DuplicateSelection,
    SelectionMismatch,
    Selection(conduit_ai::TemporalEvidenceFlowRefusal),
}
use EpisodeRetrievalWindowRefusal as R;
pub struct PreparedEpisodeRetrievalWindow {
    window_type: Vec<u8>,
    outcome_type: Vec<u8>,
    output: Vec<u8>,
    parser: PreparedEpisodeRetrievalEvidence,
    verifier: conduit_ai::BoundedTemporalEvidenceFlow,
    snapshot: Option<[u8; 32]>,
    keys: [EpisodeRetrievalCandidateKey; MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
    count: usize,
    omissions: [Option<EpisodeTemporalOmission>; MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
    omitted: usize,
    selected: [EpisodeRetrievalCandidateKey; MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
    selected_len: usize,
    resolution: Option<EpisodeRetrievalResolution>,
}
impl PreparedEpisodeRetrievalWindow {
    pub fn prepare() -> Self {
        Self {
            window_type: conduit_ai::TemporalEvidenceWindow::semantic_type()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
            outcome_type: conduit_ai::TemporalEvidenceOutcome::semantic_type()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
            output: Vec::with_capacity(MAX),
            parser: PreparedEpisodeRetrievalEvidence::new(),
            verifier: conduit_ai::BoundedTemporalEvidenceFlow::prepare(),
            snapshot: None,
            keys: [EpisodeRetrievalCandidateKey::default(); MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
            count: 0,
            omissions: [None; MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
            omitted: 0,
            selected: [EpisodeRetrievalCandidateKey::default();
                MAXIMUM_EPISODE_TEMPORAL_CANDIDATES],
            selected_len: 0,
            resolution: None,
        }
    }
    pub fn build(&mut self, snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>) -> Result<&[u8], R> {
        self.output.clear();
        self.snapshot = None;
        self.count = 0;
        self.omitted = 0;
        self.selected_len = 0;
        self.resolution = None;
        if let Err(error) = self.build_inner(snapshot) {
            self.output.clear();
            self.count = 0;
            self.omitted = 0;
            return Err(error);
        }
        self.snapshot = Some(snapshot.identity());
        Ok(&self.output)
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn candidates(&self) -> &[EpisodeRetrievalCandidateKey] {
        &self.keys[..self.count]
    }
    pub fn omissions(&self) -> impl Iterator<Item = EpisodeTemporalOmission> + '_ {
        self.omissions[..self.omitted].iter().flatten().copied()
    }
    pub fn resolution(&self) -> Option<EpisodeRetrievalResolution> {
        self.resolution
    }
    fn omit(
        &mut self,
        episode: u64,
        item: Option<u16>,
        reason: EpisodeTemporalOmissionReason,
    ) -> Result<(), R> {
        if self.omitted == MAXIMUM_EPISODE_TEMPORAL_CANDIDATES {
            return Err(R::ItemBound);
        }
        self.omissions[self.omitted] = Some(EpisodeTemporalOmission {
            episode,
            item,
            reason,
        });
        self.omitted += 1;
        Ok(())
    }
    fn build_inner(&mut self, snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>) -> Result<(), R> {
        put(&mut self.output, &self.window_type)?;
        record(&mut self.output, 4)?;
        name(&mut self.output, "candidates")?;
        put(&mut self.output, &[1])?;
        let count_at = self.output.len();
        put(&mut self.output, &0u32.to_le_bytes())?;
        let input = snapshot.input();
        for (episode, stored) in snapshot.records() {
            let evidence = field(stored, "evidence")?;
            let Some(text) = evidence.variant_payload("some").map_err(|_| R::Malformed)? else {
                self.omit(
                    episode,
                    None,
                    EpisodeTemporalOmissionReason::MissingEvidence,
                )?;
                continue;
            };
            let parsed = self.parser.parse(leaf(text)?).map_err(R::Evidence)?;
            if parsed.items().is_empty() {
                self.omit(
                    episode,
                    None,
                    EpisodeTemporalOmissionReason::MissingEvidence,
                )?;
            }
            for (item, metadata) in parsed.items().iter().enumerate() {
                let Some(instant) = metadata.observed_at else {
                    self.omit(
                        episode,
                        Some(item as u16),
                        EpisodeTemporalOmissionReason::MissingObservedTime,
                    )?;
                    continue;
                };
                let time = instant.borrowed();
                let reason =
                    if time.scale != TemporalScale::Milliseconds || time.resolution_ticks != 1 {
                        Some(EpisodeTemporalOmissionReason::UnsupportedTime)
                    } else if time.clock_basis != input.reference.clock_basis {
                        Some(EpisodeTemporalOmissionReason::ClockMismatch)
                    } else {
                        match time.relation_to(&input.reference) {
                            Err(_) => Some(EpisodeTemporalOmissionReason::InvalidInterval),
                            Ok(TemporalRelation::Future { .. }) => {
                                Some(EpisodeTemporalOmissionReason::FutureSource)
                            }
                            _ => None,
                        }
                    };
                if let Some(reason) = reason {
                    self.omit(episode, Some(item as u16), reason)?;
                    continue;
                }
                if self.count == MAXIMUM_EPISODE_TEMPORAL_CANDIDATES {
                    return Err(R::ItemBound);
                }
                let key = EpisodeRetrievalCandidateKey {
                    episode,
                    item: item as u16,
                };
                candidate(
                    &mut self.output,
                    key,
                    time.ticks,
                    time.uncertainty_ticks,
                    input,
                )?;
                self.keys[self.count] = key;
                self.count += 1;
            }
        }
        self.output[count_at..count_at + 4].copy_from_slice(&(self.count as u32).to_le_bytes());
        name(&mut self.output, "earliest_history_complete")?;
        scalar(&mut self.output, &[0])?;
        name(&mut self.output, "identity")?;
        scalar(&mut self.output, &hex_identity(snapshot.identity()))?;
        name(&mut self.output, "reference")?;
        record(&mut self.output, 2)?;
        name(&mut self.output, "clock_basis")?;
        clock(&mut self.output, input.clock_basis)?;
        name(&mut self.output, "reference_at")?;
        number(&mut self.output, input.reference.ticks)?;
        validate_canonical_structured_value(&self.output).map_err(|_| R::Malformed)?;
        Ok(())
    }
    /// Resolve only against the same immutable capture that produced this window.
    pub fn resolve(
        &mut self,
        snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
        outcome: &[u8],
    ) -> Result<&[EpisodeRetrievalCandidateKey], R> {
        self.selected_len = 0;
        self.resolution = None;
        if let Err(error) = self.resolve_inner(snapshot, outcome) {
            self.selected_len = 0;
            self.resolution = None;
            return Err(error);
        }
        Ok(&self.selected[..self.selected_len])
    }
    fn resolve_inner(
        &mut self,
        snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
        bytes: &[u8],
    ) -> Result<(), R> {
        let identity = self.snapshot.ok_or(R::NoWindow)?;
        snapshot.match_identity(&identity).map_err(R::Snapshot)?;
        if bytes.len() > MAX {
            return Err(R::ByteBound);
        }
        let outcome = validate_canonical_structured_value(bytes).map_err(|_| R::Malformed)?;
        if outcome.type_bytes() != self.outcome_type {
            return Err(R::Malformed);
        }
        if leaf(field(outcome, "window_identity")?)? != hex_identity(identity) {
            return Err(R::SnapshotMismatch);
        }
        let selection = field(outcome, "selection")?;
        if let Some(selected) = selection
            .variant_payload("selected")
            .map_err(|_| R::Malformed)?
        {
            let identities = field(selected, "identities")?;
            for index in 0..128 {
                let Some(id) = identities
                    .collection_index(index)
                    .map_err(|_| R::Malformed)?
                else {
                    break;
                };
                let id = leaf(id)?;
                let key = self.keys[..self.count]
                    .iter()
                    .copied()
                    .find(|key| {
                        let (buffer, n) = candidate_identity(*key);
                        &buffer[..n] == id
                    })
                    .ok_or(R::UnknownSelection)?;
                if self.selected[..self.selected_len].contains(&key) {
                    return Err(R::DuplicateSelection);
                }
                if self.selected_len == MAXIMUM_EPISODE_TEMPORAL_CANDIDATES {
                    return Err(R::ItemBound);
                }
                snapshot.original_record(key.episode).map_err(R::Snapshot)?;
                self.selected[self.selected_len] = key;
                self.selected_len += 1;
            }
            if self.selected_len == 0 {
                return Err(R::Malformed);
            }
            self.resolution = Some(EpisodeRetrievalResolution::Selected);
        } else {
            for (tag, status) in [
                ("no_history", EpisodeRetrievalResolution::NoHistory),
                (
                    "need_earlier_history",
                    EpisodeRetrievalResolution::NeedEarlierHistory,
                ),
                (
                    "boundary_unavailable",
                    EpisodeRetrievalResolution::BoundaryUnavailable,
                ),
            ] {
                if selection
                    .variant_payload(tag)
                    .map_err(|_| R::Malformed)?
                    .is_some()
                {
                    self.resolution = Some(status);
                    break;
                }
            }
            if self.resolution.is_none() {
                return Err(R::Malformed);
            }
        }
        // Current intent can change independently upstream. The supplied result
        // must implement the exact intent bound into this immutable capture.
        let expected = self
            .verifier
            .execute(&self.output, snapshot.input().intent)
            .map_err(R::Selection)?;
        if expected != bytes {
            return Err(R::SelectionMismatch);
        }
        Ok(())
    }
}
impl Default for PreparedEpisodeRetrievalWindow {
    fn default() -> Self {
        Self::prepare()
    }
}
