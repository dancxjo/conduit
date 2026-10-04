#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_speech::{
    neighbor_match::NeighborObservation, occurrence_context::*, rule_stress::StressObservation,
    semantic::*,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
#[test]
fn native_adjacency_checks_every_basis_axis_and_u32_edges() {
    for (earlier, later, accepted) in [
        (0, 1, true),
        (u32::MAX - 1, u32::MAX, true),
        (u32::MAX, 0, false),
        (1, 1, false),
        (1, 0, false),
        (0, 2, false),
    ] {
        let result = SpeechOccurrenceAdjacency::new(
            fixture::token(earlier, fixture::BASIS),
            fixture::token(later, fixture::BASIS),
        );
        assert_eq!(result.is_ok(), accepted, "{earlier} -> {later}");
        if !accepted {
            assert!(
                matches!(
                    &result,
                    Err(NativeBindingRefusal::ViolatedInvariant { index: 0 })
                ),
                "unexpected ordinal refusal: {result:?}"
            );
        }
        if let Ok(receipt) = result {
            let decoded = SpeechOccurrenceAdjacency::from_structured(
                receipt.clone().into_structured().unwrap(),
            )
            .unwrap();
            assert_eq!(decoded, receipt);
        }
    }
    for axis in 0..5 {
        let mut foreign = fixture::BASIS;
        foreign[axis] = if axis == 1 { "fr" } else { "foreign" };
        assert!(
            SpeechOccurrenceAdjacency::new(
                fixture::token(0, fixture::BASIS),
                fixture::token(1, foreign)
            )
            .is_err(),
            "basis axis {axis}"
        );
    }
}
#[test]
fn immediate_segments_keep_nonzero_ordinals_and_unknown_endpoints() {
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let context = resolve_intent_occurrence_context(&intent, 1).unwrap();
    assert!(core::ptr::eq(context.intent(), &intent));
    let SpeechUtteranceIntentEvent::Segment(original) = &intent.events().as_slice()[1] else {
        panic!("segment")
    };
    assert!(core::ptr::eq(context.segment(), original));
    for (neighbor, index, earlier, later) in
        [(context.before(), 0, 10, 11), (context.after(), 2, 11, 12)]
    {
        let PlannedNeighbor::Segment {
            event,
            segment,
            membership,
            adjacency,
        } = neighbor
        else {
            panic!("segment")
        };
        assert_eq!(*event, index);
        assert_eq!(adjacency.earlier().ordinal(), &earlier);
        assert_eq!(adjacency.later().ordinal(), &later);
        assert_eq!(membership.occurrence(), segment.occurrence());
        assert!(
            matches!(neighbor.stress(), StressObservation::Segment(value) if core::ptr::eq(value, segment.stress()))
        );
        assert!(
            matches!(neighbor.observation(), NeighborObservation::Segment { phone, features: None, .. } if core::ptr::eq(phone, segment.phone()))
        );
    }
    let first = resolve_intent_occurrence_context(&intent, 0).unwrap();
    let last = resolve_intent_occurrence_context(&intent, 2).unwrap();
    assert!(matches!(
        first.before().observation(),
        NeighborObservation::Unknown
    ));
    assert!(matches!(
        last.after().observation(),
        NeighborObservation::Unknown
    ));
    assert!(matches!(
        resolve_intent_occurrence_context(&intent, usize::MAX),
        Err(OccurrenceContextRefusal::MissingEvent)
    ));
}
#[test]
fn boundaries_are_retained_events_and_are_never_skipped_or_inferred() {
    for kind in [
        SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
        SpeechBoundarySpecification::unknown(),
    ] {
        let intent = fixture::intent([
            fixture::segment(10),
            fixture::boundary(kind),
            fixture::segment(11),
        ]);
        let context = resolve_intent_occurrence_context(&intent, 0).unwrap();
        let PlannedNeighbor::Boundary { event, boundary } = context.after() else {
            panic!("boundary")
        };
        assert_eq!(*event, 1);
        let SpeechUtteranceIntentEvent::Boundary(original) = &intent.events().as_slice()[1] else {
            panic!("boundary")
        };
        assert!(core::ptr::eq(*boundary, original));
        assert!(
            matches!(context.after().observation(), NeighborObservation::Boundary(value) if core::ptr::eq(value, original.kind()))
        );
        assert!(matches!(
            resolve_intent_occurrence_context(&intent, 1),
            Err(OccurrenceContextRefusal::BoundaryEvent)
        ));
        let context = resolve_intent_occurrence_context(&intent, 2).unwrap();
        assert!(matches!(
            context.before(),
            PlannedNeighbor::Boundary { event: 1, .. }
        ));
    }
}
#[test]
fn duplicate_gap_reversal_foreign_sequence_and_foreign_membership_are_refused() {
    let duplicate = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(10),
    ]);
    assert!(matches!(
        resolve_intent_occurrence_context(&duplicate, 0),
        Err(OccurrenceContextRefusal::DuplicateOccurrence {
            first: 0,
            second: 2
        })
    ));
    for ordinals in [[10, 12], [10, 9]] {
        let intent = fixture::intent(ordinals.map(fixture::segment));
        assert!(matches!(
            resolve_intent_occurrence_context(&intent, 0),
            Err(OccurrenceContextRefusal::Neighbor { event: 1, .. })
        ));
    }
    let mut foreign = fixture::BASIS;
    foreign[3] = "foreign sequence";
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment_with(fixture::token(11, foreign)),
    ]);
    assert!(matches!(
        resolve_intent_occurrence_context(&intent, 0),
        Err(OccurrenceContextRefusal::Neighbor { event: 1, .. })
    ));
    foreign = fixture::BASIS;
    foreign[2] = "foreign revision";
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment_with(fixture::token(12, foreign)),
    ]);
    assert!(matches!(
        resolve_intent_occurrence_context(&intent, 0),
        Err(OccurrenceContextRefusal::Membership(_))
    ));
}
#[test]
fn full_native_event_bound_is_traversed_without_treating_indices_as_ordinals() {
    let intent = fixture::intent((1000..1256).map(fixture::segment));
    let context = resolve_intent_occurrence_context(&intent, 128).unwrap();
    assert_eq!(context.segment().occurrence().ordinal(), &1128);
    assert!(matches!(
        context.before(),
        PlannedNeighbor::Segment { event: 127, .. }
    ));
    assert!(matches!(
        context.after(),
        PlannedNeighbor::Segment { event: 129, .. }
    ));
    let intent = fixture::intent(
        (1000..1255)
            .map(fixture::segment)
            .chain([fixture::segment(1000)]),
    );
    assert!(matches!(
        resolve_intent_occurrence_context(&intent, 128),
        Err(OccurrenceContextRefusal::DuplicateOccurrence {
            first: 0,
            second: 255
        })
    ));
}
