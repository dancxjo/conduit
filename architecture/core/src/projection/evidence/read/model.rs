//! Borrowed wire claims for inspection, never an admitted semantic report.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionEvidenceReadRefusal {
    Extent,
    Malformed,
    UnsupportedVersion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionEvidenceFact<'a> {
    Preserved {
        obligation: &'a str,
    },
    Transformed {
        obligation: &'a str,
        law: &'a str,
    },
    Lost {
        obligation: &'a str,
        class: ProjectionLoss,
        detail: &'a [u8],
        native_fact: Option<&'a str>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEvidenceNative<'a> {
    pub identity: &'a str,
    pub contract: &'a str,
    pub provider: &'a str,
    pub encoding: &'a str,
    pub bytes: &'a [u8],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEvidenceScoreValue<'a> {
    pub scale: &'a str,
    pub calibration: Option<&'a str>,
    pub producer: &'a str,
    pub minimum: i64,
    pub maximum: i64,
    pub value: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionEvidenceScore<'a> {
    Native {
        fact: &'a str,
        score: ProjectionEvidenceScoreValue<'a>,
    },
    Normalized {
        fact: &'a str,
        original: ProjectionEvidenceScoreValue<'a>,
        projected: ProjectionEvidenceScoreValue<'a>,
        policy: &'a str,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEvidenceRealization<'a> {
    pub plan: &'a str,
    pub host: &'a str,
    pub boot: &'a str,
    pub back: &'a str,
    pub artifact: &'a str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEvidenceAttempt<'a> {
    pub realization: ProjectionEvidenceRealization<'a>,
    pub outcome: ProjectionMechanism,
    pub fallback_reason: Option<&'a str>,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Section<'a> {
    pub bytes: &'a [u8],
    pub count: u8,
}
/// Complete generic wire inventory with opaque domain payloads. Successful
/// decoding proves bounded framing only, not policy/domain validity or execution.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionEvidenceView<'a> {
    pub codec: &'a str,
    pub source_contract: &'a str,
    pub target_contract: &'a str,
    pub policy: &'a str,
    pub configuration_digest: &'a [u8; 32],
    pub report: &'a str,
    pub source_identity: &'a str,
    pub target_identity: Option<&'a str>,
    pub projector: &'a str,
    pub requested_route: &'a str,
    pub boundary: Option<(&'a str, &'a str)>,
    pub policy_bytes: &'a [u8],
    pub source_bytes: &'a [u8],
    pub target_bytes: Option<&'a [u8]>,
    pub selected_attempt: Option<usize>,
    pub mechanism: ProjectionMechanism,
    pub claimed_summary: ProjectionSummary,
    pub(super) facts: Section<'a>,
    pub(super) native: Section<'a>,
    pub(super) scores: Section<'a>,
    pub(super) admitted: Section<'a>,
    pub(super) attempts: Section<'a>,
    pub(super) diagnostics: Section<'a>,
}
/// Iteration replays already checked immutable framing; no allocation occurs.
pub struct ProjectionEvidenceItems<'a, T> {
    pub(super) cursor: Cursor<'a>,
    pub(super) remaining: u8,
    pub(super) parse: fn(&mut Cursor<'a>) -> Result<T, ProjectionEvidenceReadRefusal>,
}
impl<T> Iterator for ProjectionEvidenceItems<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some((self.parse)(&mut self.cursor).expect("immutable previously checked evidence framing"))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = usize::from(self.remaining);
        (n, Some(n))
    }
}
impl<T> ExactSizeIterator for ProjectionEvidenceItems<'_, T> {}
impl<T> core::iter::FusedIterator for ProjectionEvidenceItems<'_, T> {}
