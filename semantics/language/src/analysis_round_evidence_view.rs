//! Borrowed evidence inspection for hosted and retained complete-round receipts.
//! No Native decoding, encoding, numerical copy, or implicit allocation.
use super::{cursor, FallbackOperationObservation};
use alloc::{string::String, vec::Vec};

pub(super) enum RoundEvidence {
    Hosted {
        context: [String; 5],
        observations: Vec<FallbackOperationObservation>,
        #[cfg(all(
            feature = "parser-model",
            target_has_atomic = "ptr",
            target_has_atomic = "8"
        ))]
        learned: Vec<super::learned::LearnedBranchEvidence>,
    },
    Retained(cursor::retained_evidence::RetainedRoundEvidence),
}
impl RoundEvidence {
    pub(super) fn context(&self) -> &[String; 5] {
        match self {
            Self::Hosted { context, .. } => context,
            Self::Retained(evidence) => evidence.context(),
        }
    }
    pub(super) fn observations(&self) -> RoundObservations<'_> {
        RoundObservations { evidence: self }
    }
    /// None means retained evidence, NOT an empty hosted learned history.
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub(super) fn hosted_learned(&self) -> Option<&[super::learned::LearnedBranchEvidence]> {
        match self {
            Self::Hosted { learned, .. } => Some(learned),
            Self::Retained(_) => None,
        }
    }
    pub(super) fn retained(&self) -> Option<&cursor::retained_evidence::RetainedRoundEvidence> {
        match self {
            Self::Hosted { .. } => None,
            Self::Retained(evidence) => Some(evidence),
        }
    }
}

/// A borrowed observation. Hosted frames do not carry individual Native
/// admission flags; retained frames expose their actual recorded flag.
#[derive(Clone, Copy)]
pub struct RoundObservation<'a> {
    pub operation: &'a str,
    pub input: &'a [u8],
    pub output: &'a [u8],
    pub native_admitted: Option<bool>,
}

/// Zero-copy adapter over either owned frames or an exact retained arena.
#[derive(Clone, Copy)]
pub struct RoundObservations<'a> {
    evidence: &'a RoundEvidence,
}
impl<'a> RoundObservations<'a> {
    pub fn len(&self) -> usize {
        match self.evidence {
            RoundEvidence::Hosted { observations, .. } => observations.len(),
            RoundEvidence::Retained(evidence) => evidence.observation_count(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<RoundObservation<'a>> {
        match self.evidence {
            RoundEvidence::Hosted { observations, .. } => {
                let frame = observations.get(index)?;
                Some(RoundObservation {
                    operation: &frame.operation,
                    input: &frame.input,
                    output: &frame.output,
                    native_admitted: None,
                })
            }
            RoundEvidence::Retained(evidence) => {
                let frame = evidence.observation(index)?;
                Some(RoundObservation {
                    operation: frame.operation,
                    input: frame.input,
                    output: frame.output,
                    native_admitted: Some(frame.admitted),
                })
            }
        }
    }
    pub fn iter(&self) -> RoundObservationIter<'a> {
        RoundObservationIter {
            observations: *self,
            next: 0,
        }
    }
    /// Explicit ALLOCATING diagnostic export for post-Play hosted replay.
    /// Copies all operation names and byte buffers. The legacy output type has
    /// no per-frame Native flag; this export does not transfer admission proof.
    pub fn to_vec(&self) -> Vec<FallbackOperationObservation> {
        self.iter()
            .map(|frame| FallbackOperationObservation {
                operation: String::from(frame.operation),
                input: frame.input.to_vec(),
                output: frame.output.to_vec(),
            })
            .collect()
    }
}

pub struct RoundObservationIter<'a> {
    observations: RoundObservations<'a>,
    next: usize,
}
impl<'a> Iterator for RoundObservationIter<'a> {
    type Item = RoundObservation<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.observations.get(self.next)?;
        self.next += 1;
        Some(frame)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.observations.len() - self.next;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for RoundObservationIter<'_> {}
impl core::iter::FusedIterator for RoundObservationIter<'_> {}
impl<'a> IntoIterator for RoundObservations<'a> {
    type Item = RoundObservation<'a>;
    type IntoIter = RoundObservationIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
