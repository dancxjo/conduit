//! Finite boundary-oriented selection over exact temporal evidence.

use alloc::{vec, vec::Vec};
use conduit_form::rust_binding::BoundedSequence;

use crate::{
    TemporalEvidenceBatch, TemporalEvidenceCandidate, TemporalEvidenceIdentities,
    TemporalEvidenceSelection, TemporalEvidenceSelectionRefusal, TemporalRetrievalIntent,
};

pub const MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES: usize = 128;
pub const MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES: usize = 256;

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
        for candidate in self.candidates.get() {
            let instant = candidate
                .provenance
                .source_instant(candidate.source)
                .map_err(|_| TemporalEvidenceSelectionRefusal::MissingSourceTime)?;
            if candidate_matches(candidate, instant, intent) {
                matches.push((instant, candidate.identity.clone()));
            }
        }
        matches.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.get().cmp(right.1.get()))
        });

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
            | TemporalRetrievalIntent::DurationSince(_) => {
                vec![matches[0].1.clone()]
            }
            TemporalRetrievalIntent::LatestEvidence => {
                vec![matches[matches.len() - 1].1.clone()]
            }
            _ => matches.into_iter().map(|(_, identity)| identity).collect(),
        };
        Ok(TemporalEvidenceSelection::selected(
            TemporalEvidenceIdentities::new(
                BoundedSequence::try_from_iter(identities)
                    .expect("selection cannot exceed the checked 128-candidate batch"),
            )
            .expect("selection always contains at least one identity"),
        )
        .expect("a checked identity sequence is a valid selection"))
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
        if self.candidates.get().is_empty() {
            return Err(TemporalEvidenceSelectionRefusal::EmptyCandidates);
        }
        if self.candidates.get().len() > MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES {
            return Err(TemporalEvidenceSelectionRefusal::TooManyCandidates);
        }
        for (index, candidate) in self.candidates.get().iter().enumerate() {
            if candidate.identity.get().is_empty() {
                return Err(TemporalEvidenceSelectionRefusal::EmptyIdentity);
            }
            if candidate.identity.get().len() > MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES {
                return Err(TemporalEvidenceSelectionRefusal::IdentityTooLarge);
            }
            if self.candidates.get().as_slice()[index + 1..]
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
        TemporalRetrievalIntent::StateValidAt(payload) => {
            let query_instant = *payload.instant();
            candidate
                .provenance
                .valid_from
                .is_some_and(|start| start <= query_instant)
                && candidate
                    .provenance
                    .valid_until
                    .is_none_or(|end| query_instant <= end)
        }
        TemporalRetrievalIntent::Transition(payload) => {
            candidate.transition == Some(*payload.direction())
        }
        TemporalRetrievalIntent::DurationSince(payload) => {
            candidate.boundary == Some(*payload.boundary())
        }
        TemporalRetrievalIntent::EvidenceWithin(payload) => {
            *payload.window().start() <= instant && instant <= *payload.window().end()
        }
    }
}

fn needs_earlier_history(intent: &TemporalRetrievalIntent) -> bool {
    matches!(
        intent,
        TemporalRetrievalIntent::EarliestEvidence
            | TemporalRetrievalIntent::StateValidAt(_)
            | TemporalRetrievalIntent::Transition(_)
            | TemporalRetrievalIntent::DurationSince(_)
            | TemporalRetrievalIntent::EvidenceWithin(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockBasis, EntityBoundary, TemporalEvidenceCandidates, TemporalEvidenceIdentity,
        TemporalProvenance, TemporalReference, TemporalSource, TemporalValidity,
    };
    use alloc::{format, string::String, vec};
    use conduit_form::rust_binding::NativeRustBinding;

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
            identity: TemporalEvidenceIdentity::new(identity.into()).unwrap(),
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
            candidates: TemporalEvidenceCandidates::new(
                BoundedSequence::try_from_iter(candidates).unwrap(),
            )
            .unwrap(),
            earliest_history_complete: complete,
        }
    }

    fn selected(identities: &[&str]) -> TemporalEvidenceSelection {
        TemporalEvidenceSelection::selected(
            TemporalEvidenceIdentities::new(
                BoundedSequence::try_from_iter(
                    identities
                        .iter()
                        .map(|identity| TemporalEvidenceIdentity::new((*identity).into()).unwrap()),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn temporal_evidence_family_is_native_and_exactly_bounded() {
        let evidence = candidate("evidence/one", 100);
        let structured = evidence.clone().into_structured().unwrap();
        assert_eq!(
            TemporalEvidenceCandidate::from_structured(structured).unwrap(),
            evidence
        );
        let batch = batch(vec![evidence], true);
        let structured = batch.clone().into_structured().unwrap();
        assert_eq!(
            TemporalEvidenceBatch::from_structured(structured).unwrap(),
            batch
        );
        let selection = selected(&["evidence/one"]);
        let structured = selection.clone().into_structured().unwrap();
        assert_eq!(
            TemporalEvidenceSelection::from_structured(structured).unwrap(),
            selection
        );
        assert_eq!(
            serde_json::from_str::<TemporalEvidenceSelection>(
                &serde_json::to_string(&selection).unwrap()
            )
            .unwrap(),
            selection
        );
        assert_eq!(
            serde_json::to_value(&selection).unwrap(),
            serde_json::json!({"Selected": {"identities": ["evidence/one"]}})
        );
        assert!(TemporalEvidenceIdentity::new(String::new()).is_err());
        assert!(TemporalEvidenceIdentity::new("x".repeat(256)).is_ok());
        assert!(TemporalEvidenceIdentity::new("x".repeat(257)).is_err());
        assert!(TemporalEvidenceCandidates::new(BoundedSequence::new()).is_err());
        assert!(BoundedSequence::<_, 128>::try_from_iter(
            (0..129).map(|index| candidate(&format!("evidence/{index}"), index))
        )
        .is_err());
    }

    #[test]
    fn recent_first_page_cannot_masquerade_as_project_origin() {
        let recent = batch(vec![candidate("summary/recent", 900)], false);
        assert_eq!(
            recent.select(&TemporalRetrievalIntent::EarliestEvidence),
            Ok(TemporalEvidenceSelection::NeedEarlierHistory)
        );
        assert_eq!(
            recent.select(
                &TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap(),
            ),
            Ok(TemporalEvidenceSelection::NeedEarlierHistory)
        );
    }

    #[test]
    fn exact_boundary_and_latest_evidence_are_selected_deterministically() {
        let mut origin = candidate("project/created", 100);
        origin.boundary = Some(EntityBoundary::Created);
        let evidence = batch(vec![candidate("summary/recent", 900), origin], true);
        assert_eq!(
            evidence.select(
                &TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap(),
            ),
            Ok(selected(&["project/created"]))
        );
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::LatestEvidence),
            Ok(selected(&["summary/recent"]))
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
            evidence.select(&TemporalRetrievalIntent::state_valid_at(300).unwrap()),
            Ok(selected(&["state/B"]))
        );
        assert_eq!(
            evidence.select(&TemporalRetrievalIntent::EventOrdering),
            Ok(selected(&["state/A", "state/B", "state/C"]))
        );
    }

    #[test]
    fn complete_history_reports_missing_boundary_without_fabricating_duration() {
        assert_eq!(
            batch(vec![candidate("summary/recent", 900)], true)
                .select(&TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap()),
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
