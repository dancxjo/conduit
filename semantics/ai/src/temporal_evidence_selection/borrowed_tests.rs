extern crate std;
use super::tests::{batch, candidate};
use super::*;
use crate::{EntityBoundary, TemporalRetrievalWindow, TransitionDirection};
use alloc::{vec, vec::Vec};

// The probe is thread scoped, so unrelated unit tests do not affect the count.
#[allow(dead_code)]
mod allocation_probe {
    use super::std;
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn compare(batch: &crate::TemporalEvidenceBatch, intent: &TemporalRetrievalIntent) {
    let candidates: Vec<_> = batch
        .candidates
        .get()
        .iter()
        .map(BorrowedTemporalEvidenceCandidate::from)
        .collect();
    let mut selected = TemporalEvidenceIndexSelection::new();
    select_temporal_evidence_into(
        (&batch.reference).into(),
        &candidates,
        intent,
        batch.earliest_history_complete,
        &mut selected,
    )
    .unwrap();
    let mut primitive = TemporalEvidenceIndexSelection::new();
    let result = assert_without_allocations(|| {
        select_borrowed_temporal_evidence_into(
            (&batch.reference).into(),
            &candidates,
            intent.into(),
            batch.earliest_history_complete,
            &mut primitive,
        )
    });
    result.unwrap();
    assert_eq!(primitive, selected);
    let owned = batch.select(intent).unwrap();
    match owned {
        TemporalEvidenceSelection::Selected(v) => {
            assert_eq!(selected.status(), TemporalEvidenceSelectionStatus::Selected);
            let actual: Vec<_> = selected
                .indices()
                .iter()
                .map(|&i| candidates[i].identity)
                .collect();
            let expected: Vec<_> = v
                .identities()
                .get()
                .iter()
                .map(|i| i.get().as_str())
                .collect();
            assert_eq!(actual, expected);
        }
        TemporalEvidenceSelection::NeedEarlierHistory => assert_eq!(
            selected.status(),
            TemporalEvidenceSelectionStatus::NeedEarlierHistory
        ),
        TemporalEvidenceSelection::BoundaryUnavailable => assert_eq!(
            selected.status(),
            TemporalEvidenceSelectionStatus::BoundaryUnavailable
        ),
    }
}
#[test]
fn all_intents_share_owned_results_with_ties_and_uncertainty() {
    let mut a = candidate("a", 100);
    a.provenance.uncertainty_millis = Some(90);
    a.provenance.valid_until = Some(199);
    a.boundary = Some(EntityBoundary::Created);
    let mut b = candidate("b", 100);
    b.provenance.valid_until = Some(150);
    let mut c = candidate("c", 200);
    c.transition = Some(TransitionDirection::IntoState);
    let intents = [
        TemporalRetrievalIntent::EarliestEvidence,
        TemporalRetrievalIntent::LatestEvidence,
        TemporalRetrievalIntent::EventOrdering,
        TemporalRetrievalIntent::state_valid_at(175).unwrap(),
        TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap(),
        TemporalRetrievalIntent::transition(TransitionDirection::IntoState).unwrap(),
        TemporalRetrievalIntent::evidence_within(TemporalRetrievalWindow::new(90, 110).unwrap())
            .unwrap(),
    ];
    for complete in [false, true] {
        let batch = batch(vec![c.clone(), b.clone(), a.clone()], complete);
        for intent in &intents {
            compare(&batch, intent);
        }
    }
}
#[test]
fn empty_is_explicit_and_refusals_clear_stale_output() {
    let data = batch(vec![candidate("a", 100)], true);
    let base = BorrowedTemporalEvidenceCandidate::from(&data.candidates.get().as_slice()[0]);
    let reference = (&data.reference).into();
    let intent = TemporalRetrievalIntent::LatestEvidence;
    let mut out = TemporalEvidenceIndexSelection::new();
    select_temporal_evidence_into(reference, &[base], &intent, true, &mut out).unwrap();
    assert_eq!(out.indices(), &[0]);
    select_temporal_evidence_into(reference, &[], &intent, false, &mut out).unwrap();
    assert_eq!(out.status(), TemporalEvidenceSelectionStatus::NoHistory);
    assert!(out.indices().is_empty());
    let mut invalid = base;
    invalid.identity = "";
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::EmptyIdentity)
    );
    assert!(out.indices().is_empty());
    let mut invalid = base;
    invalid.provenance.clock_basis = BorrowedTemporalClockBasis::MonotonicMilliseconds("other");
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::ReferenceMismatch)
    );
    let mut invalid = base;
    invalid.provenance.valid_from = Some(2);
    invalid.provenance.valid_until = Some(1);
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::InvalidProvenance)
    );
    let mut invalid = base;
    invalid.provenance.event_at = None;
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::MissingSourceTime)
    );
    assert_eq!(
        select_temporal_evidence_into(reference, &[base, base], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::DuplicateIdentity)
    );
    assert_eq!(
        select_temporal_evidence_into(reference, &[base; 129], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::TooManyCandidates)
    );
}
#[test]
fn maximum_batch_selects_without_allocation() {
    let data = batch(
        (0..128)
            .map(|i| candidate(&alloc::format!("candidate/{i:03}"), 127 - i))
            .collect(),
        true,
    );
    let borrowed: Vec<_> = data
        .candidates
        .get()
        .iter()
        .map(BorrowedTemporalEvidenceCandidate::from)
        .collect();
    let mut out = TemporalEvidenceIndexSelection::new();
    let (result, allocation) = allocation_probe::observe(|| {
        select_temporal_evidence_into(
            (&data.reference).into(),
            &borrowed,
            &TemporalRetrievalIntent::EventOrdering,
            true,
            &mut out,
        )
    });
    result.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(out.indices().len(), 128);
    assert_eq!(out.indices()[0], 127);
    assert_eq!(out.indices()[127], 0);
}

#[test]
fn borrowed_reference_and_identity_bounds_are_checked() {
    let data = batch(vec![candidate("a", 100)], true);
    let base = BorrowedTemporalEvidenceCandidate::from(&data.candidates.get().as_slice()[0]);
    let reference = BorrowedTemporalReference::from(&data.reference);
    let mut out = TemporalEvidenceIndexSelection::new();
    let intent = TemporalRetrievalIntent::LatestEvidence;
    let long = alloc::string::String::from_utf8(vec![b'x'; 257]).unwrap();
    let mut invalid = base;
    invalid.identity = &long;
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::IdentityTooLarge)
    );
    let mut invalid = base;
    invalid.provenance.retrieved_at = reference.reference_at + 1;
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::InvalidProvenance)
    );
    let mut invalid = base;
    invalid.provenance.reference_at += 1;
    assert_eq!(
        select_temporal_evidence_into(reference, &[invalid], &intent, true, &mut out),
        Err(crate::TemporalEvidenceSelectionRefusal::ReferenceMismatch)
    );
}

#[test]
fn ties_have_exact_identity_order_and_boundary_completeness_is_explicit() {
    let data = batch(vec![candidate("z", 100), candidate("a", 100)], false);
    let candidates: Vec<_> = data
        .candidates
        .get()
        .iter()
        .map(BorrowedTemporalEvidenceCandidate::from)
        .collect();
    let reference = (&data.reference).into();
    let mut out = TemporalEvidenceIndexSelection::new();
    select_temporal_evidence_into(
        reference,
        &candidates,
        &TemporalRetrievalIntent::EventOrdering,
        false,
        &mut out,
    )
    .unwrap();
    assert_eq!(out.indices(), &[1, 0]);
    select_temporal_evidence_into(
        reference,
        &candidates,
        &TemporalRetrievalIntent::LatestEvidence,
        false,
        &mut out,
    )
    .unwrap();
    assert_eq!(out.indices(), &[0]);
    select_temporal_evidence_into(
        reference,
        &candidates,
        &TemporalRetrievalIntent::EarliestEvidence,
        false,
        &mut out,
    )
    .unwrap();
    assert_eq!(
        out.status(),
        TemporalEvidenceSelectionStatus::NeedEarlierHistory
    );
    let intent = TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap();
    select_temporal_evidence_into(reference, &candidates, &intent, false, &mut out).unwrap();
    assert_eq!(
        out.status(),
        TemporalEvidenceSelectionStatus::NeedEarlierHistory
    );
    select_temporal_evidence_into(reference, &candidates, &intent, true, &mut out).unwrap();
    assert_eq!(
        out.status(),
        TemporalEvidenceSelectionStatus::BoundaryUnavailable
    );
}

#[test]
fn clock_comparison_preserves_reference_validation_and_exact_variant_identity() {
    let data = batch(vec![candidate("a", 100)], true);
    let mut item = BorrowedTemporalEvidenceCandidate::from(&data.candidates.get().as_slice()[0]);
    let mut reference = BorrowedTemporalReference::from(&data.reference);
    let mut out = TemporalEvidenceIndexSelection::new();
    // Existing TemporalReference::validate imposes no label syntax rule.
    // Equal borrowed labels therefore remain equal, even for an empty label.
    reference.clock_basis = BorrowedTemporalClockBasis::MonotonicMilliseconds("");
    item.provenance.clock_basis = reference.clock_basis;
    select_temporal_evidence_into(
        reference,
        &[item],
        &TemporalRetrievalIntent::LatestEvidence,
        true,
        &mut out,
    )
    .unwrap();
    assert_eq!(out.indices(), &[0]);
    item.provenance.clock_basis = BorrowedTemporalClockBasis::MonotonicMilliseconds("clock/A");
    assert_eq!(
        select_temporal_evidence_into(
            reference,
            &[item],
            &TemporalRetrievalIntent::LatestEvidence,
            true,
            &mut out
        ),
        Err(crate::TemporalEvidenceSelectionRefusal::ReferenceMismatch)
    );
    reference.clock_basis = BorrowedTemporalClockBasis::UnixEpochMilliseconds;
    item.provenance.clock_basis =
        BorrowedTemporalClockBasis::MonotonicMilliseconds(conduit_core::UNIX_UTC_CLOCK_BASIS);
    assert_eq!(
        select_temporal_evidence_into(
            reference,
            &[item],
            &TemporalRetrievalIntent::LatestEvidence,
            true,
            &mut out
        ),
        Err(crate::TemporalEvidenceSelectionRefusal::ReferenceMismatch)
    );
}

pub(super) fn assert_without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    let (result, allocation) = allocation_probe::observe(operation);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    result
}

#[test]
fn primitive_reversed_window_refuses_before_empty_or_candidate_processing() {
    let data = batch(vec![candidate("a", 100)], true);
    let item = BorrowedTemporalEvidenceCandidate::from(&data.candidates.get().as_slice()[0]);
    let reference = (&data.reference).into();
    let intent = BorrowedTemporalRetrievalIntent::EvidenceWithin { start: 20, end: 10 };
    let mut output = TemporalEvidenceIndexSelection::new();
    for candidates in [&[item][..], &[][..]] {
        let result = assert_without_allocations(|| {
            select_borrowed_temporal_evidence_into(reference, candidates, intent, true, &mut output)
        });
        assert_eq!(
            result,
            Err(crate::TemporalEvidenceSelectionRefusal::InvalidIntent)
        );
        assert!(output.indices().is_empty());
    }
}
