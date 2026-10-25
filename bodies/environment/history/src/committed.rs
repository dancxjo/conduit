//! Exact local publication commit -> historical Face. This does not run storage.
use crate::{HistoryFaceRefusal, PreparedHistoryFace, HISTORY_SUBJECT};
use conduit_core::{BoundedResourceRef, ResourceReferenceRefusal};
use conduit_environment_runtime::{
    observation::ObservationContract,
    recording::{RecordingCommitObserver, RecordingContext, RecoveredHistory},
};
use conduit_face::{FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue as V};

/// One admitted view of at most 4096 samples in the existing 64-sample chunks.
/// A longer recording or another view needs its own explicit preparation.
pub const MAXIMUM_COMMITTED_HISTORY_GENERATIONS: usize = 64;
#[derive(Debug, PartialEq, Eq)]
pub enum CommittedHistoryRefusal {
    Empty,
    Capacity,
    DuplicateGeneration,
    Reference(ResourceReferenceRefusal),
    Uncommitted,
    ContentMismatch,
    History(HistoryFaceRefusal),
}

/// Root retains the exact publication observers supplied by the actual selected
/// Backs before Play. Reading cannot substitute a caller-provided status flag,
/// sequence range or reference. The existing history reader still validates the
/// independently supplied producer context and ordered chunk boundary.
///
/// A successful read establishes these local Backs' kernel publication commits,
/// not file durability, a catalog/seed commit, Body membership, transport trust,
/// disclosure authority or fresh-process recovery. Those retain their existing
/// independent admission and proof obligations.
pub struct PreparedCommittedHistoryFace<'a> {
    history: PreparedHistoryFace,
    publications: &'a [RecordingCommitObserver],
    next: usize,
}
impl<'a> PreparedCommittedHistoryFace<'a> {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        publications: &'a [RecordingCommitObserver],
    ) -> Result<Self, CommittedHistoryRefusal> {
        if publications.is_empty() {
            return Err(CommittedHistoryRefusal::Empty);
        }
        if publications.len() > MAXIMUM_COMMITTED_HISTORY_GENERATIONS {
            return Err(CommittedHistoryRefusal::Capacity);
        }
        let mut generations = [None; MAXIMUM_COMMITTED_HISTORY_GENERATIONS];
        for (index, publication) in publications.iter().enumerate() {
            let reference = BoundedResourceRef::validate_encoded(publication.encoded_reference())
                .map_err(CommittedHistoryRefusal::Reference)?;
            let generation = Some((reference.identity, reference.version));
            // Changing extent, profile or expiry cannot turn the same immutable
            // semantic generation into a second publication slot.
            if generations[..index].contains(&generation) {
                return Err(CommittedHistoryRefusal::DuplicateGeneration);
            }
            generations[index] = generation;
        }
        Ok(Self {
            history: PreparedHistoryFace::prepare(context, contract, publications.len() as u16)
                .map_err(CommittedHistoryRefusal::History)?,
            publications,
            next: 0,
        })
    }
    /// Immutable identity data of the next selected generation. This is neither
    /// a storage read permit nor evidence that publication has committed.
    pub fn next_reference(&self) -> Option<&[u8]> {
        self.publications
            .get(self.next)
            .map(RecordingCommitObserver::encoded_reference)
    }
    /// Refusals preserve the prior visible chunk and the next selected observer.
    /// Even valid content with identical sample counters cannot replace the
    /// exact original input retained by that Back's committed content digest.
    pub fn read_next(&mut self, json: &str) -> Result<(), CommittedHistoryRefusal> {
        let publication = self
            .publications
            .get(self.next)
            .ok_or(CommittedHistoryRefusal::Capacity)?;
        if publication.committed().is_none() {
            return Err(CommittedHistoryRefusal::Uncommitted);
        }
        if !publication.committed_content_matches(json.as_bytes()) {
            return Err(CommittedHistoryRefusal::ContentMismatch);
        }
        self.history
            .read_next(json)
            .map_err(CommittedHistoryRefusal::History)?;
        self.next += 1;
        Ok(())
    }
    pub fn recovered(&self) -> RecoveredHistory {
        self.history.recovered()
    }
    /// This allocating preparation projects existing committed content; it does
    /// not acknowledge a Show or perform a kernel/storage operation.
    pub fn contribution(
        &mut self,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, CommittedHistoryRefusal> {
        let mut fragment = self
            .history
            .contribution(basis)
            .map_err(CommittedHistoryRefusal::History)?;
        fragment.properties.push(FaceProperty {
            subject: HISTORY_SUBJECT.into(),
            name: "publication-evidence".into(),
            value: V::Text(
                if self.next == 0 {
                    "none"
                } else {
                    "kernel-publication-commit"
                }
                .into(),
            ),
        });
        if let Some(publication) = self
            .next
            .checked_sub(1)
            .and_then(|index| self.publications.get(index))
        {
            fragment.properties.push(FaceProperty {
                subject: HISTORY_SUBJECT.into(),
                name: "last-publication-reference".into(),
                value: V::Content(publication.encoded_reference().to_vec()),
            });
        }
        fragment
            .validate_bounds()
            .map_err(|_| CommittedHistoryRefusal::History(HistoryFaceRefusal::Fragment))?;
        Ok(fragment)
    }
}
