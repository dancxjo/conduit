#[allow(dead_code)] // Shared evidence fixtures serve independent contracts.
#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_core::{
    BorrowedTemporalInstant, TemporalInstant, TemporalRelationError, TemporalScale,
};
use conduit_human::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_semantic_catalog::*;
use conduit_time::NativeTemporalInstant;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct CountingAllocator;
thread_local! {static COUNTING:Cell<bool>=const{Cell::new(false)};static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNTING.with(|f| {
            if f.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, s: usize) -> *mut u8 {
        COUNTING.with(|f| {
            if f.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(p, l, s) }
    }
}
fn instant(ticks: u64, uncertainty: u64) -> NativeTemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "sensor/clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: uncertainty,
    }
    .try_into()
    .unwrap()
}
fn time(at: NativeTemporalInstant) -> ExperienceSituationTime {
    ExperienceSituationTime::at(at).unwrap()
}

fn absent() -> ExperienceSituationTime {
    ExperienceSituationTime::Absent
}
fn frame(
    values: Vec<(ExperienceEvidenceItem, ExperienceSituationTime)>,
    reference: NativeTemporalInstant,
) -> Vec<u8> {
    let ids: Vec<_> = values
        .iter()
        .map(|(v, _)| v.identity().get().as_str())
        .collect();
    let selected = selection(&ids);
    let observations = ExperienceSituationObservations::new(
        BoundedSequence::try_from_iter(
            values
                .into_iter()
                .map(|(v, t)| ExperienceSituationObservation::new(v, t).unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    encode(
        ExperienceSituationFrame::new(
            observations,
            ExperienceTemporalPolicy::new(10, 20).unwrap(),
            reference,
            true,
            selected,
        )
        .unwrap(),
    )
}
#[test]
fn explicit_time_classification_preserves_content_sources_and_both_outputs() {
    let reference = instant(100, 0);
    let input = frame(
        vec![
            (observation("current"), time(instant(95, 0))),
            (observation("recent"), time(instant(85, 0))),
            (observation("stale"), time(instant(70, 0))),
        ],
        reference.clone(),
    );
    let mut situation = BoundedExperienceSituation::prepare();
    situation.execute(&input).unwrap();
    let request = ExperienceEvidenceRequest::decode(situation.situation()).unwrap();
    let items = request.evidence().get().as_slice();
    assert_eq!(
        items.iter().map(|i| *i.temporal_role()).collect::<Vec<_>>(),
        vec![
            ExperienceTemporalRole::Current,
            ExperienceTemporalRole::Recent,
            ExperienceTemporalRole::Stale
        ]
    );
    for item in items {
        assert_eq!(item.origin(), &ExperienceOrigin::Observation);
        assert_eq!(item.encoded_content(), observation("x").encoded_content());
        assert_eq!(
            &item.sources().get().as_slice()[0],
            &observation("x").sources().get().as_slice()[0]
        );
        assert_eq!(item.sources().get().as_slice().len(), 2);
    }
    let event = ExperienceAutobiographyEvent::decode(situation.event()).unwrap();
    assert_eq!(event.observation(), &request);
    assert!(*event.retain());
    let view = conduit_core::validate_canonical_structured_value(situation.situation()).unwrap();
    let temporal = view
        .record_field("evidence")
        .unwrap()
        .unwrap()
        .collection_index(0)
        .unwrap()
        .unwrap()
        .record_field("sources")
        .unwrap()
        .unwrap()
        .collection_index(1)
        .unwrap()
        .unwrap()
        .variant_payload("temporal_context")
        .unwrap()
        .unwrap();
    assert_eq!(
        temporal
            .record_field("reference_at")
            .unwrap()
            .unwrap()
            .value_node(),
        conduit_core::validate_canonical_structured_value(&encode(reference))
            .unwrap()
            .value_node()
    );
    assert_eq!(
        BorrowedTemporalInstant::from_canonical(
            temporal.record_field("observed_at").unwrap().unwrap()
        )
        .unwrap()
        .ticks,
        95
    );
}
#[test]
fn uncertainty_across_freshness_boundary_is_indeterminate_and_uncertain() {
    let original = ExperienceEvidenceItem::new(
        ExperienceAvailability::Present,
        ExperienceCertainty::Certain,
        identity("value/text"),
        ExperienceDomain::HumanUtterance,
        observation("x").encoded_content().clone(),
        identity("uncertain"),
        ExperienceOrigin::Observation,
        identity("provenance/7"),
        7,
        observation("x").sources().clone(),
        ExperienceTemporalRole::Current,
    )
    .unwrap();
    let input = frame(vec![(original, time(instant(89, 2)))], instant(100, 0));
    let mut situation = BoundedExperienceSituation::prepare();
    situation.execute(&input).unwrap();
    let request = ExperienceEvidenceRequest::decode(situation.situation()).unwrap();
    let item = &request.evidence().get().as_slice()[0];
    assert_eq!(item.temporal_role(), &ExperienceTemporalRole::Indeterminate);
    assert_eq!(item.certainty(), &ExperienceCertainty::Uncertain);
}
#[test]
fn absent_sources_stay_empty_uncertain_and_have_no_fabricated_time() {
    let missing = item(
        "missing",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Stale,
        ExperienceAvailability::NotObserved,
        7,
        "provenance/7",
        b"",
    );
    let unavailable = item(
        "unavailable",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Stale,
        ExperienceAvailability::SourceUnavailable,
        7,
        "provenance/7",
        b"",
    );
    let input = frame(
        vec![(missing, absent()), (unavailable, absent())],
        instant(100, 0),
    );
    let mut situation = BoundedExperienceSituation::prepare();
    situation.execute(&input).unwrap();
    let request = ExperienceEvidenceRequest::decode(situation.situation()).unwrap();
    for item in request.evidence().get().as_slice() {
        assert_ne!(item.availability(), &ExperienceAvailability::Present);
        assert_eq!(item.temporal_role(), &ExperienceTemporalRole::Current);
        assert_eq!(item.certainty(), &ExperienceCertainty::Uncertain);
        assert_eq!(item.sources().get().as_slice().len(), 1);
        assert!(item.encoded_content().get().as_slice().is_empty());
    }
}
#[test]
fn future_incomparable_overflow_recollection_and_source_bound_refuse_atomically() {
    let mut situation = BoundedExperienceSituation::prepare();
    let valid = frame(
        vec![(observation("x"), time(instant(100, 0)))],
        instant(100, 0),
    );
    situation.execute(&valid).unwrap();
    let future = frame(
        vec![(observation("x"), time(instant(101, 0)))],
        instant(100, 0),
    );
    assert_eq!(
        situation.execute(&future),
        Err(ExperienceSituationRefusal::FutureObservation)
    );
    assert!(situation.event().is_empty() && situation.situation().is_empty());
    let other = TemporalInstant {
        ticks: 100,
        scale: TemporalScale::Milliseconds,
        clock_basis: "other/clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
    .try_into()
    .unwrap();
    assert_eq!(
        situation.execute(&frame(
            vec![(observation("x"), time(other))],
            instant(100, 0)
        )),
        Err(ExperienceSituationRefusal::Temporal(
            TemporalRelationError::Incomparable
        ))
    );
    assert_eq!(
        situation.execute(&frame(
            vec![(observation("x"), time(instant(0, 1)))],
            instant(100, 0)
        )),
        Err(ExperienceSituationRefusal::Temporal(
            TemporalRelationError::IntervalOverflow
        ))
    );
    let recalled = item(
        "x",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"recalled",
    );
    assert_eq!(
        situation.execute(&frame(
            vec![(recalled, time(instant(100, 0)))],
            instant(100, 0)
        )),
        Err(ExperienceSituationRefusal::RememberedInput)
    );
    let full = with_sources(
        observation("x"),
        (0..4)
            .map(|n| ExperienceEvidenceSource::source(identity(&format!("source/{n}"))).unwrap())
            .collect(),
    );
    assert_eq!(
        situation.execute(&frame(vec![(full, time(instant(100, 0)))], instant(100, 0))),
        Err(ExperienceSituationRefusal::SourceBound)
    );
    assert_eq!(
        situation.execute(&frame(vec![(observation("x"), absent())], instant(100, 0))),
        Err(ExperienceSituationRefusal::InvalidAvailability)
    );
    assert!(situation.event().is_empty() && situation.situation().is_empty());
}
#[test]
fn owned_and_borrowed_temporal_relations_share_exact_behavior() {
    for (ticks, uncertainty) in [(95, 0), (100, 0), (101, 0), (99, 2), (0, 1)] {
        let owned = TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "sensor/clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: uncertainty,
        };
        let reference = TemporalInstant {
            ticks: 100,
            uncertainty_ticks: 0,
            ..owned.clone()
        };
        assert_eq!(
            owned.relation_to(&reference),
            owned.borrowed().relation_to(&reference.borrowed())
        );
    }
}
#[test]
fn execution_and_atomic_refusal_allocate_nothing() {
    let input = frame(
        vec![(observation("current"), time(instant(95, 0)))],
        instant(100, 0),
    );
    let future = frame(
        vec![(observation("future"), time(instant(101, 0)))],
        instant(100, 0),
    );
    let mut situation = BoundedExperienceSituation::prepare();
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|f| f.set(true));
    let valid = situation.execute(&input).is_ok();
    let refused = situation.execute(&future) == Err(ExperienceSituationRefusal::FutureObservation);
    let empty = situation.event().is_empty() && situation.situation().is_empty();
    let recovery = situation.execute(&input).is_ok();
    COUNTING.with(|f| f.set(false));
    assert!(valid && refused && empty && recovery);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn overlapping_time_within_current_window_keeps_reviewed_freshness_policy() {
    let observed = instant(100, 1);
    let reference = instant(100, 1);
    let policy = ExperienceTemporalPolicy::new(10, 20).unwrap();
    assert_eq!(
        policy
            .classify(
                &observed.clone().try_into().unwrap(),
                &reference.clone().try_into().unwrap()
            )
            .unwrap(),
        ExperienceTemporalRole::Current
    );
    let input = frame(vec![(observation("overlap"), time(observed))], reference);
    let mut selector = BoundedExperienceSituation::prepare();
    selector.execute(&input).unwrap();
    let selected = ExperienceEvidenceRequest::decode(selector.situation()).unwrap();
    assert_eq!(
        selected.evidence().get().as_slice()[0].temporal_role(),
        &ExperienceTemporalRole::Current
    );
}
