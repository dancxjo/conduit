//! Finite boundary-oriented selection over exact temporal evidence.

use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    TemporalEvidenceBatch, TemporalEvidenceIdentities, TemporalEvidenceSelection,
    TemporalEvidenceSelectionRefusal, TemporalRetrievalIntent,
};

pub const MAXIMUM_TEMPORAL_EVIDENCE_CANDIDATES: usize = 128;
pub const MAXIMUM_TEMPORAL_EVIDENCE_IDENTITY_BYTES: usize = 256;

mod borrowed;
pub use borrowed::*;

impl TemporalEvidenceBatch {
    pub fn select(
        &self,
        intent: &TemporalRetrievalIntent,
    ) -> Result<TemporalEvidenceSelection, TemporalEvidenceSelectionRefusal> {
        let mut output = TemporalEvidenceIndexSelection::new();
        borrowed::select_indices(
            BorrowedTemporalReference::from(&self.reference),
            self.candidates.get().len(),
            |index| {
                BorrowedTemporalEvidenceCandidate::from(&self.candidates.get().as_slice()[index])
            },
            intent.into(),
            self.earliest_history_complete,
            false,
            &mut output,
        )?;
        match output.status() {
            TemporalEvidenceSelectionStatus::NeedEarlierHistory => {
                Ok(TemporalEvidenceSelection::NeedEarlierHistory)
            }
            TemporalEvidenceSelectionStatus::BoundaryUnavailable => {
                Ok(TemporalEvidenceSelection::BoundaryUnavailable)
            }
            TemporalEvidenceSelectionStatus::NoHistory => {
                unreachable!("owned empty batches refuse")
            }
            TemporalEvidenceSelectionStatus::Selected => {
                let identities = output
                    .indices()
                    .iter()
                    .map(|&index| self.candidates.get().as_slice()[index].identity.clone());
                Ok(TemporalEvidenceSelection::selected(
                    TemporalEvidenceIdentities::new(
                        BoundedSequence::try_from_iter(identities)
                            .expect("checked candidate bound"),
                    )
                    .expect("nonempty selection"),
                )
                .expect("checked identities"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockBasis, EntityBoundary, TemporalEvidenceCandidate, TemporalEvidenceCandidates,
        TemporalEvidenceIdentity, TemporalProvenance, TemporalReference, TemporalSource,
        TemporalValidity,
    };
    use alloc::{format, string::String, vec, vec::Vec};
    use conduit_plot::rust_binding::NativeRustBinding;

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

    pub(super) fn candidate(identity: &str, event_at: u64) -> TemporalEvidenceCandidate {
        TemporalEvidenceCandidate {
            identity: TemporalEvidenceIdentity::new(identity.into()).unwrap(),
            provenance: provenance(event_at, None),
            source: TemporalSource::Event,
            boundary: None,
            transition: None,
            validity: TemporalValidity::Current,
        }
    }

    pub(super) fn batch(
        candidates: Vec<TemporalEvidenceCandidate>,
        complete: bool,
    ) -> TemporalEvidenceBatch {
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

#[cfg(test)]
#[path = "temporal_evidence_selection/borrowed_tests.rs"]
mod borrowed_tests;

/// Shared thread-scoped unit-test allocation assertion; one test allocator owns
/// the AI unit-test binary, including other bounded semantic implementations.
#[cfg(test)]
pub(crate) fn assert_without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    borrowed_tests::assert_without_allocations(operation)
}
