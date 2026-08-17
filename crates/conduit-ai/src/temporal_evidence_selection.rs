//! Finite boundary-oriented selection over exact temporal evidence.

use alloc::{string::String, vec, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    EntityBoundary, TemporalProvenance, TemporalReference, TemporalRetrievalIntent, TemporalSource,
    TemporalValidity, TransitionDirection,
};

pub const MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES: usize = 128;
pub const MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalEvidenceCandidate {
    pub identity: String,
    pub provenance: TemporalProvenance,
    pub source: TemporalSource,
    pub boundary: Option<EntityBoundary>,
    pub transition: Option<TransitionDirection>,
    pub validity: TemporalValidity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalEvidenceBatch {
    pub reference: TemporalReference,
    pub candidates: Vec<TemporalEvidenceCandidate>,
    /// True only when no earlier evidence page exists for this retrieval scope.
    pub earliest_history_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalEvidenceSelection {
    Selected { identities: Vec<String> },
    NeedEarlierHistory,
    BoundaryUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalEvidenceSelectionRefusal {
    EmptyCandidates,
    TooManyCandidates,
    EmptyIdentity,
    IdentityTooLarge,
    DuplicateIdentity,
    InvalidReference,
    InvalidIntent,
    InvalidProvenance,
    ReferenceMismatch,
    MissingSourceTime,
}

impl TemporalEvidenceBatch {
    pub fn select(
        &self,
        intent: &TemporalRetrievalIntent,
    ) -> Result<TemporalEvidenceSelection, TemporalEvidenceSelectionRefusal> {
        self.validate(intent)?;
        if matches!(intent, TemporalRetrievalIntent::EarliestEvidence)
            && !self.earliest_history_complete
        {
            return Ok(TemporalEvidenceSelection::NeedEarlierHistory);
        }

        let mut matches = Vec::new();
        for candidate in &self.candidates {
            let instant = candidate
                .provenance
                .source_instant(candidate.source)
                .map_err(|_| TemporalEvidenceSelectionRefusal::MissingSourceTime)?;
            if candidate_matches(candidate, instant, intent) {
                matches.push((instant, candidate.identity.clone()));
            }
        }
        matches.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        if matches.is_empty() {
            return Ok(
                if needs_earlier_history(intent) && !self.earliest_history_complete {
                    TemporalEvidenceSelection::NeedEarlierHistory
                } else {
                    TemporalEvidenceSelection::BoundaryUnavailable
                },
            );
        }

        let identities = match intent {
            TemporalRetrievalIntent::EarliestEvidence
            | TemporalRetrievalIntent::DurationSince { .. } => {
                vec![matches[0].1.clone()]
            }
            TemporalRetrievalIntent::LatestEvidence => {
                vec![matches[matches.len() - 1].1.clone()]
            }
            _ => matches.into_iter().map(|(_, identity)| identity).collect(),
        };
        Ok(TemporalEvidenceSelection::Selected { identities })
    }

    fn validate(
        &self,
        intent: &TemporalRetrievalIntent,
    ) -> Result<(), TemporalEvidenceSelectionRefusal> {
        self.reference
            .validate()
            .map_err(|_| TemporalEvidenceSelectionRefusal::InvalidReference)?;
        intent
            .validate()
            .map_err(|_| TemporalEvidenceSelectionRefusal::InvalidIntent)?;
        if self.candidates.is_empty() {
            return Err(TemporalEvidenceSelectionRefusal::EmptyCandidates);
        }
        if self.candidates.len() > MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES {
            return Err(TemporalEvidenceSelectionRefusal::TooManyCandidates);
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            if candidate.identity.is_empty() {
                return Err(TemporalEvidenceSelectionRefusal::EmptyIdentity);
            }
            if candidate.identity.len() > MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES {
                return Err(TemporalEvidenceSelectionRefusal::IdentityTooLarge);
            }
            if self.candidates[index + 1..]
                .iter()
                .any(|other| other.identity == candidate.identity)
            {
                return Err(TemporalEvidenceSelectionRefusal::DuplicateIdentity);
            }
            candidate
                .provenance
                .validate()
                .map_err(|_| TemporalEvidenceSelectionRefusal::InvalidProvenance)?;
            if candidate.provenance.reference_at != self.reference.reference_at
                || candidate.provenance.clock_basis != self.reference.clock_basis
            {
                return Err(TemporalEvidenceSelectionRefusal::ReferenceMismatch);
            }
            candidate
                .provenance
                .source_instant(candidate.source)
                .map_err(|_| TemporalEvidenceSelectionRefusal::MissingSourceTime)?;
        }
        Ok(())
    }
}

fn candidate_matches(
    candidate: &TemporalEvidenceCandidate,
    instant: u64,
    intent: &TemporalRetrievalIntent,
) -> bool {
    match intent {
        TemporalRetrievalIntent::EarliestEvidence
        | TemporalRetrievalIntent::LatestEvidence
        | TemporalRetrievalIntent::EventOrdering => true,
        TemporalRetrievalIntent::StateValidAt { instant } => {
            candidate
                .provenance
                .valid_from
                .is_some_and(|start| start <= *instant)
                && candidate
                    .provenance
                    .valid_until
                    .is_none_or(|end| *instant <= end)
        }
        TemporalRetrievalIntent::Transition { direction } => {
            candidate.transition == Some(*direction)
        }
        TemporalRetrievalIntent::DurationSince { boundary } => {
            candidate.boundary == Some(*boundary)
        }
        TemporalRetrievalIntent::EvidenceWithin { start, end } => {
            *start <= instant && instant <= *end
        }
    }
}

fn needs_earlier_history(intent: &TemporalRetrievalIntent) -> bool {
    matches!(
        intent,
        TemporalRetrievalIntent::EarliestEvidence
            | TemporalRetrievalIntent::StateValidAt { .. }
            | TemporalRetrievalIntent::Transition { .. }
            | TemporalRetrievalIntent::DurationSince { .. }
            | TemporalRetrievalIntent::EvidenceWithin { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClockBasis;
    use alloc::{string::ToString, vec};

    fn provenance(event_at: u64, valid_until: Option<u64>) -> TemporalProvenance {
        TemporalProvenance {
            event_at: Some(event_at),
            valid_from: Some(event_at),
            valid_until,
            observed_at: Some(event_at + 1),
            recorded_at: Some(event_at + 2),
            ingested_at: Some(event_at + 3),
            retrieved_at: 900,
            reference_at: 1_000,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
            uncertainty_millis: None,
        }
    }

    fn candidate(identity: &str, event_at: u64) -> TemporalEvidenceCandidate {
        TemporalEvidenceCandidate {
            identity: identity.to_string(),
            provenance: provenance(event_at, None),
            source: TemporalSource::Event,
            boundary: None,
            transition: None,
            validity: TemporalValidity::Current,
        }
    }

    fn batch(candidates: Vec<TemporalEvidenceCandidate>, complete: bool) -> TemporalEvidenceBatch {
        TemporalEvidenceBatch {
            reference: TemporalReference {
                reference_at: 1_000,
                clock_basis: ClockBasis::UnixEpochMilliseconds,
            },
            candidates,
            earliest_history_complete: complete,
        }
    }

    #[test]
    fn recent_first_page_cannot_masquerade_as_project_origin() {
        let recent = batch(vec![candidate("summary/recent", 900)], false);
        assert_eq!(
            recent.select(&TemporalRetrievalIntent::EarliestEvidence),
            Ok(TemporalEvidenceSelection::NeedEarlierHistory)
        );
        assert_eq!(
            recent.select(&TemporalRetrievalIntent::DurationSince {
                boundary: EntityBoundary::Created,
            }),
            Ok(TemporalEvidenceSelection::NeedEarlierHistory)
        );
    }

    #[test]
    fn exact_boundary_and_latest_evidence_are_selected_deterministically() {
        let mut origin = candidate("project/created", 100);
        origin.boundary = Some(EntityBoundary::Created);
        let evidence = batch(vec![candidate("summary/recent", 900), origin], true);
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::DurationSince {
                boundary: EntityBoundary::Created,
            }),
            Ok(TemporalEvidenceSelection::Selected {
                identities: vec!["project/created".to_string()]
            })
        );
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::LatestEvidence),
            Ok(TemporalEvidenceSelection::Selected {
                identities: vec!["summary/recent".to_string()]
            })
        );
    }

    #[test]
    fn state_interval_and_event_ordering_use_event_and_validity_truth() {
        let mut state_b = candidate("state/B", 200);
        state_b.provenance.valid_until = Some(400);
        state_b.validity = TemporalValidity::Historical;
        let mut state_a = candidate("state/A", 100);
        state_a.provenance.valid_until = Some(199);
        state_a.validity = TemporalValidity::Historical;
        let evidence = batch(vec![candidate("state/C", 401), state_b, state_a], true);
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::StateValidAt { instant: 300 }),
            Ok(TemporalEvidenceSelection::Selected {
                identities: vec!["state/B".to_string()]
            })
        );
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::EventOrdering),
            Ok(TemporalEvidenceSelection::Selected {
                identities: vec![
                    "state/A".to_string(),
                    "state/B".to_string(),
                    "state/C".to_string(),
                ]
            })
        );
    }

    #[test]
    fn complete_history_reports_missing_boundary_without_fabricating_duration() {
        assert_eq!(
            batch(vec![candidate("summary/recent", 900)], true).select(
                &TemporalRetrievalIntent::DurationSince {
                    boundary: EntityBoundary::Created,
                }
            ),
            Ok(TemporalEvidenceSelection::BoundaryUnavailable)
        );
    }

    #[test]
    fn malformed_batch_refuses_before_selection() {
        let duplicate = candidate("same", 100);
        assert_eq!(
            batch(vec![duplicate.clone(), duplicate], true)
                .select(&TemporalRetrievalIntent::LatestEvidence),
            Err(TemporalEvidenceSelectionRefusal::DuplicateIdentity)
        );
        let mut mismatch = candidate("mismatch", 100);
        mismatch.provenance.reference_at = 999;
        assert_eq!(
            batch(vec![mismatch], true).select(&TemporalRetrievalIntent::LatestEvidence),
            Err(TemporalEvidenceSelectionRefusal::ReferenceMismatch)
        );
        let mut missing = candidate("missing", 100);
        missing.source = TemporalSource::ValidUntil;
        assert_eq!(
            batch(vec![missing], true).select(&TemporalRetrievalIntent::LatestEvidence),
            Err(TemporalEvidenceSelectionRefusal::MissingSourceTime)
        );
    }
}
