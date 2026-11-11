//! Context selection over verified temporal results, using actual retained
//! episode projections. Ranks are structural ordinals, never relevance scores.
use crate::{
    BorrowedEpisodeRetrievalSnapshot, EpisodeRetrievalCandidateKey,
    EpisodeRetrievalProjectionRefusal, EpisodeRetrievalResolution, EpisodeRetrievalWindowRefusal,
    PreparedEpisodeRetrievalProjection, PreparedEpisodeRetrievalWindow,
};
use alloc::vec::Vec;
use conduit_ai::*;
use conduit_core::{
    validate_canonical_structured_value, BorrowedTemporalInstant, TemporalScale,
    ValidatedCanonicalStructuredValue,
};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "episode_retrieval_context/custody.rs"]
mod custody;
use custody::*;
pub const EPISODE_CONTEXT_BYTE_CHARGE_PROFILE: &str = "context/utf8-json-byte-charge@1";
const EPISODES: usize = 4;
const ITEMS: usize = 32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalContextRefusal {
    UnsupportedAccounting,
    PolicyMismatch,
    Window(EpisodeRetrievalWindowRefusal),
    Projection(EpisodeRetrievalProjectionRefusal),
    Context(ContextSelectionRefusal),
    Malformed,
    ItemBound,
    SourceSpan,
}
use EpisodeRetrievalContextRefusal as R;
#[derive(Clone, Copy, Default)]
struct Group {
    episode: u64,
    representative: usize,
    indices: [usize; 8],
    len: usize,
}
/// Prepared episode-local history workspace. It is not another memory store.
pub struct PreparedEpisodeRetrievalContext {
    policy: ContextSelectionPolicy,
    policy_bytes: Vec<u8>,
    window: PreparedEpisodeRetrievalWindow,
    projector: PreparedEpisodeRetrievalProjection,
    selector: PreparedContextSelector,
    fragments: [Vec<u8>; EPISODES],
    identities: [[u8; 64]; ITEMS],
    id_lengths: [usize; ITEMS],
    groups: [Group; EPISODES],
    selected: [usize; EPISODES],
    omissions: [Option<BorrowedContextOmission>; EPISODES],
    group_count: usize,
    result: Option<BorrowedContextSelection>,
    temporal: Option<EpisodeRetrievalResolution>,
    selected_snapshot: Option<[u8; 32]>,
}
impl PreparedEpisodeRetrievalContext {
    /// Token charge is one declared unit per rendered UTF-8 byte. It is not a
    /// model-tokenizer measurement. The caller separately bounds the full model
    /// request, including current evidence and instruction/envelope overhead.
    pub fn prepare(policy: ContextSelectionPolicy) -> Result<Self, R> {
        if policy.token_accounting_profile() != EPISODE_CONTEXT_BYTE_CHARGE_PROFILE {
            return Err(R::UnsupportedAccounting);
        }
        let policy_bytes = policy.clone().encode().map_err(|_| R::Malformed)?;
        Ok(Self {
            policy,
            policy_bytes,
            window: PreparedEpisodeRetrievalWindow::prepare(),
            projector: PreparedEpisodeRetrievalProjection::prepare(),
            selector: PreparedContextSelector::prepare(EPISODES).map_err(R::Context)?,
            fragments: core::array::from_fn(|_| {
                Vec::with_capacity(crate::MAXIMUM_EPISODE_RETRIEVAL_PROJECTION_BYTES)
            }),
            identities: [[0; 64]; ITEMS],
            id_lengths: [0; ITEMS],
            groups: [Group::default(); EPISODES],
            selected: [usize::MAX; EPISODES],
            omissions: [None; EPISODES],
            group_count: 0,
            result: None,
            temporal: None,
            selected_snapshot: None,
        })
    }
    pub fn temporal_omissions(&self) -> impl Iterator<Item = crate::EpisodeTemporalOmission> + '_ {
        self.window.omissions()
    }
    pub fn temporal_resolution(&self) -> Option<EpisodeRetrievalResolution> {
        self.temporal
    }
    pub fn selection(&self) -> Option<BorrowedContextSelection> {
        self.result
    }
    /// Identifies the capture used by the last successful selection, including
    /// an explicitly empty temporal result. Cleared on every refusal.
    pub fn selected_snapshot_identity(&self) -> Option<[u8; 32]> {
        self.selected_snapshot
    }
    pub fn selected_episodes(&self) -> impl Iterator<Item = u64> + '_ {
        self.selected[..self.result.map_or(0, |r| r.selected_len)]
            .iter()
            .map(|i| self.groups[*i].episode)
    }
    pub fn selected_fragments(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.selected[..self.result.map_or(0, |r| r.selected_len)]
            .iter()
            .map(|i| self.fragments[*i].as_slice())
    }
    pub fn omissions(&self) -> impl Iterator<Item = (u64, ContextOmissionReason)> + '_ {
        self.omissions
            .iter()
            .flatten()
            .map(|o| (self.groups[o.candidate_index].episode, o.reason))
    }
    pub fn select(
        &mut self,
        snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
        outcome: &[u8],
    ) -> Result<Option<BorrowedContextSelection>, R> {
        self.result = None;
        self.selected_snapshot = None;
        self.temporal = None;
        self.group_count = 0;
        self.selected.fill(usize::MAX);
        self.omissions.fill(None);
        for fragment in &mut self.fragments {
            fragment.clear();
        }
        match self.select_inner(snapshot, outcome) {
            Ok(result) => {
                self.result = result;
                self.selected_snapshot = Some(snapshot.identity());
                Ok(result)
            }
            Err(error) => {
                self.temporal = None;
                self.selected.fill(usize::MAX);
                self.omissions.fill(None);
                for fragment in &mut self.fragments {
                    fragment.clear();
                }
                Err(error)
            }
        }
    }
    fn select_inner(
        &mut self,
        snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
        outcome: &[u8],
    ) -> Result<Option<BorrowedContextSelection>, R> {
        if snapshot.input().context_policy != self.policy_bytes {
            return Err(R::PolicyMismatch);
        }
        self.window.build(snapshot).map_err(R::Window)?;
        let mut keys = [EpisodeRetrievalCandidateKey::default(); ITEMS];
        let resolved = self.window.resolve(snapshot, outcome).map_err(R::Window)?;
        let key_count = resolved.len();
        keys[..key_count].copy_from_slice(resolved);
        self.temporal = self.window.resolution();
        if key_count == 0 {
            return Ok(None);
        }
        for (index, key) in keys[..key_count].iter().enumerate() {
            self.id_lengths[index] = key.write_identity(&mut self.identities[index]).len();
            let group = if let Some(i) = self.groups[..self.group_count]
                .iter()
                .position(|g| g.episode == key.episode)
            {
                i
            } else {
                if self.group_count == EPISODES {
                    return Err(R::ItemBound);
                }
                let i = self.group_count;
                self.groups[i] = Group {
                    episode: key.episode,
                    representative: index,
                    ..Group::default()
                };
                self.group_count += 1;
                i
            };
            let g = &mut self.groups[group];
            if g.len == 8 {
                return Err(R::ItemBound);
            }
            g.indices[g.len] = index;
            g.len += 1;
        }
        for (index, group) in self.groups[..self.group_count].iter().enumerate() {
            let projection = self
                .projector
                .render(snapshot, group.episode)
                .map_err(R::Projection)?;
            self.fragments[index].extend_from_slice(projection);
        }
        let window =
            validate_canonical_structured_value(self.window.output()).map_err(|_| R::Malformed)?;
        let entries = field(window, "candidates")?;
        let mut contributions = [[None; 8]; EPISODES];
        let mut views = [None; EPISODES];
        for (index, group) in self.groups[..self.group_count].iter().enumerate() {
            for (slot, source) in contributions[index]
                .iter_mut()
                .zip(group.indices[..group.len].iter())
            {
                *slot = Some(
                    core::str::from_utf8(&self.identities[*source][..self.id_lengths[*source]])
                        .map_err(|_| R::Malformed)?,
                );
            }
        }
        for (index, group) in self.groups[..self.group_count].iter().enumerate() {
            let representative = keys[group.representative];
            let candidate_index = self
                .window
                .candidates()
                .iter()
                .position(|key| *key == representative)
                .ok_or(R::Malformed)?;
            let candidate = entries
                .collection_index(candidate_index as u16)
                .map_err(|_| R::Malformed)?
                .ok_or(R::Malformed)?;
            let temporal = temporal(
                candidate,
                snapshot,
                contributions[index][0].ok_or(R::Malformed)?,
            )?;
            let record = snapshot
                .original_record(group.episode)
                .map_err(|error| R::Window(EpisodeRetrievalWindowRefusal::Snapshot(error)))?;
            let (start, end) = source_span(snapshot.input().memory, record.value_node())?;
            let lineage = BorrowedContextLineage {
                source: BorrowedContextSource::Owned(snapshot.input().source),
                span_unit: SourceSpanUnit::Bytes,
                span_start: start,
                span_end: end,
                extraction_profile: "extract/episode-native-record@1",
                transform_profiles: &["render/episode-history-json@1"],
                parent_chunk: None,
            };
            let digest = lineage.identity_digest().map_err(R::Context)?;
            views[index] = Some(BorrowedContextCandidate {
                chunk: BorrowedContextChunk::PreparedDigest {
                    identity: digest,
                    lineage,
                    value: BorrowedContextValue::Text(&self.fragments[index]),
                },
                reranked_rank: (index + 1) as u16,
                original_rank: (group.representative + 1) as u16,
                contributions: BorrowedContextContributions::Explicit(
                    &contributions[index][..group.len],
                ),
                reranking_policy_identity: "temporal-first-selected-episode-order@1",
                temporal: Some(temporal),
                redundancy_group: Some(
                    temporal
                        .evidence_identity
                        .split("/evidence/")
                        .next()
                        .ok_or(R::Malformed)?,
                ),
                token_count: self.fragments[index].len() as u32,
            });
        }
        let mut candidates = [views[0].ok_or(R::Malformed)?; EPISODES];
        for (slot, view) in candidates.iter_mut().zip(views[..self.group_count].iter()) {
            *slot = view.ok_or(R::Malformed)?;
        }
        self.selector
            .select(
                &self.policy,
                &candidates[..self.group_count],
                &mut self.selected,
                &mut self.omissions,
            )
            .map(Some)
            .map_err(R::Context)
    }
}
