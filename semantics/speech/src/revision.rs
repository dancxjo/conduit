//! ASR's native partial/replacement records under shared bounded lifecycle law.
//! These cursors count Unicode scalars, never UTF-8 bytes or audio frames.
use crate::semantic::*;
use conduit_core::revision::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScalarFrontier(pub u64);
pub enum ListeningDelta {
    Partial(AsrPartialHypothesis),
    Replacement(AsrRevisedHypothesis),
    Correction(AsrRevisedHypothesis),
    Withdrawal {
        range: ListeningTextRange,
        cancelled: AsrHypothesisCancelled,
    },
}
pub struct ListeningRevisions<'a> {
    pub segment: &'a ListeningSegmentId,
}
impl ListeningRevisions<'_> {
    /// Native commitment asserts already interpreted text; it does not silently
    /// replace it or attest display, synthesis, queueing or playback.
    pub fn committed_frontier(
        &self,
        commit: &AsrCommittedSegment,
        current: &str,
    ) -> Result<ScalarFrontier, RevisionRefusal> {
        if commit.segment_id() != self.segment
            || *commit.role() != ListeningTextRole::Recognition
            || commit.text() != current
        {
            return Err(RevisionRefusal::Domain);
        }
        let through = ScalarFrontier(current.chars().count() as u64);
        if self.validate_cursor(through) {
            Ok(through)
        } else {
            Err(RevisionRefusal::Domain)
        }
    }
}
impl RevisionDomain for ListeningRevisions<'_> {
    type Delta = ListeningDelta;
    type Cursor = ScalarFrontier;
    fn contract(&self) -> RevisionText<'_> {
        RevisionText::new("speech/listening-scalar-revision@1").expect("constant contract")
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &ListeningDelta) -> bool {
        let (segment, text_role) = match (role, delta) {
            (RevisionDeltaRole::Proposal, ListeningDelta::Partial(value)) => {
                (value.segment_id(), value.role())
            }
            (RevisionDeltaRole::Revision, ListeningDelta::Replacement(value))
            | (RevisionDeltaRole::Correction, ListeningDelta::Correction(value)) => {
                (value.segment_id(), value.role())
            }
            (RevisionDeltaRole::Withdrawal, ListeningDelta::Withdrawal { cancelled, .. }) => {
                (cancelled.segment_id(), cancelled.role())
            }
            _ => return false,
        };
        segment == self.segment && *text_role == ListeningTextRole::Recognition
    }
    fn validate_cursor(&self, cursor: ScalarFrontier) -> bool {
        cursor.0 <= 4096
    }
    fn region(&self, delta: &ListeningDelta) -> (ScalarFrontier, ScalarFrontier) {
        let (start, end) = match delta {
            ListeningDelta::Partial(value) => (0, value.text().chars().count() as u64),
            ListeningDelta::Replacement(value) | ListeningDelta::Correction(value) => {
                let start = u64::from(*value.replaces().start());
                (
                    start,
                    core::cmp::max(
                        u64::from(*value.replaces().end()),
                        start + value.text().chars().count() as u64,
                    ),
                )
            }
            ListeningDelta::Withdrawal { range, .. } => {
                (u64::from(*range.start()), u64::from(*range.end()))
            }
        };
        (ScalarFrontier(start), ScalarFrontier(end))
    }
    fn distance(&self, start: ScalarFrontier, end: ScalarFrontier) -> Option<u64> {
        end.0.checked_sub(start.0)
    }
}
