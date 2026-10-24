//! Retained semantic state for one admitted local publication stream. This is a
//! Face owner, not another sensor, scheduler, recorder or source of authority.
use crate::publication::PublicationSnapshot;
use conduit_environment_face::{PreparedObservationFace, PublicationRefusal};
use conduit_face::{Face, FaceBasis};

#[derive(Debug, PartialEq, Eq)]
pub enum RetainedPublicationRefusal {
    MissingExecutionBasis,
    Sequence { expected: u64, actual: u64 },
    SequenceExhausted,
    Structured(conduit_core::StructuredInfoRefusal),
    Publication(PublicationRefusal),
}
impl RetainedPublicationRefusal {
    pub const fn code(&self) -> &'static str {
        use RetainedPublicationRefusal as E;
        match self {
            E::MissingExecutionBasis => "basis",
            E::Sequence { .. } => "sequence",
            E::SequenceExhausted => "sequence-exhausted",
            E::Structured(_) => "structured",
            E::Publication(PublicationRefusal::ClockReversed) => "clock-reversed",
            E::Publication(PublicationRefusal::Exhausted) => "exhausted",
            E::Publication(_) => "publication",
        }
    }
}
/// Channel Types are linked; all mutable Face storage is prepared before Play. Root still
/// supplies its actual admitted basis, publication limit, clock and age policy.
/// Retaining claimed IDs alone establishes neither admission nor membership.
pub struct RetainedPublicationFace {
    face: PreparedObservationFace,
    last_sequence: u64,
}
impl RetainedPublicationFace {
    pub fn prepare(
        basis: FaceBasis,
        maximum_publications: u32,
        stale_after_ms: u64,
    ) -> Result<Self, RetainedPublicationRefusal> {
        use RetainedPublicationRefusal as E;
        if basis.source_document_id.is_none()
            || basis.checked_plot_id.is_none()
            || basis.expanded_plot_id.is_none()
            || basis.plan_id.is_none()
            || basis.active_play_id.is_none()
            || basis.body_id.is_some() != basis.wake_id.is_some()
        {
            return Err(E::MissingExecutionBasis);
        }
        Ok(Self {
            face: PreparedObservationFace::prepare_observed(
                basis,
                maximum_publications,
                stale_after_ms,
            )
            .map_err(E::Publication)?,
            last_sequence: 0,
        })
    }
    /// The actual local producer supplies its monotonically committed sequence
    /// and commit clock. Remote arrival time is not a substitute for either.
    /// A gap, replay, wrong channel/type or exhausted bound leaves state intact.
    pub fn publish(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        committed_ms: u64,
    ) -> Result<(), RetainedPublicationRefusal> {
        use RetainedPublicationRefusal as E;
        let expected = self
            .last_sequence
            .checked_add(1)
            .ok_or(E::SequenceExhausted)?;
        if snapshot.sequence != expected {
            return Err(E::Sequence {
                expected,
                actual: snapshot.sequence,
            });
        }
        snapshot
            .channel
            .validate_value(snapshot.canonical)
            .map_err(E::Structured)?;
        self.face
            .publish_result(snapshot.channel.index(), snapshot.canonical, committed_ms)
            .map_err(E::Publication)?;
        self.last_sequence = snapshot.sequence;
        Ok(())
    }
    pub fn last_source_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub fn current(&self) -> &Face {
        self.face.current()
    }
    /// Refresh only explicit freshness truth. This acquires no observation and
    /// advances no source sequence; the sealed Face revision may change.
    pub fn at(&mut self, now_ms: u64) -> Result<&Face, PublicationRefusal> {
        self.face.at(now_ms)
    }
}
