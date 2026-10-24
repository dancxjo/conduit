#![cfg(feature = "semantic-bindings")]
#[path = "common/phone_gestures/fixture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{phone_gestures::*, semantic::*};
use fixture::*;
#[test]
fn same_g1_owner_selects_overlapping_three_formants_energy_and_stop_phases() {
    let f = Fixture::new(false);
    let owner = f.owner();
    let controls = controls(&owner);
    let profile = PhoneGestureProfile::prepare(&owner, "G2/tata@1", &controls).unwrap();
    let gestures =
        prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), capacity()).unwrap();
    assert!(core::ptr::eq(gestures.original(), &owner));
    assert_eq!(gestures.phones().len(), 12);
    for pair in gestures.phones().windows(2) {
        assert_eq!(
            pair[0].phases.trailing_start(),
            pair[1].phases.leading_start()
        );
        assert_eq!(pair[0].phases.trailing_end(), pair[1].phases.leading_end());
        assert_eq!(pair[0].next_segment, Some(pair[1].segment));
        assert_eq!(pair[1].previous_segment, Some(pair[0].segment));
    }
    let first = &gestures.phones()[0];
    assert_eq!(first.resonance().len(), 3);
    let original = &owner.source().events()[first.event];
    let SpeechUtteranceIntentEvent::Segment(segment) = original else {
        panic!()
    };
    assert!(core::ptr::eq(
        gestures
            .selected(first)
            .unwrap()
            .choice()
            .occurrence()
            .segment(),
        segment
    ));
    assert_eq!(
        gestures
            .selected(first)
            .unwrap()
            .definition()
            .identity()
            .get()
            .as_str(),
        "g1/aspirated-t"
    );
    let phase = first.closure_release_aspiration();
    let phase_query =
        AudioTrajectoryQuery::new(anchor(), first.phases.nominal_start().clone()).unwrap();
    let phase_receipt = phase
        .evaluate(&phase_query.clone().encode().unwrap())
        .unwrap();
    assert_eq!(phase_receipt.original().segments().len(), 3);
    for (at, expected) in [
        (first.phases.nominal_start(), 0),
        (first.phases.closure_end(), 1),
        (first.phases.release_end(), 1),
    ] {
        let q = AudioTrajectoryQuery::new(anchor(), at.clone()).unwrap();
        assert_eq!(
            phase.evaluate(&q.encode().unwrap()).unwrap().result(),
            &AudioTrajectoryQuantity::amplitude(1, expected).unwrap()
        );
    }
    // Same actual neighbor transition in all three formants; no detached curve.
    for (n, expected) in [500, 1690, 2725].into_iter().enumerate() {
        let query =
            AudioTrajectoryQuery::new(anchor(), AudioExactTimeOffset::new(600, 20).unwrap())
                .unwrap();
        let left = gestures.phones()[0].resonance()[n]
            .evaluate(&query.clone().encode().unwrap())
            .unwrap();
        let right = gestures.phones()[1].resonance()[n]
            .evaluate(&query.clone().encode().unwrap())
            .unwrap();
        assert_eq!(left.result(), right.result());
        let AudioTrajectoryQuantity::Frequency(value) = left.result() else {
            panic!("frequency")
        };
        assert_eq!(*value.numerator_hz(), *value.denominator() * expected);
        // Exact Source result fractions remain unreduced; do not normalize them.
    }
    let query =
        AudioTrajectoryQuery::new(anchor(), AudioExactTimeOffset::new(600, 20).unwrap()).unwrap();
    let left = gestures.phones()[0]
        .energy()
        .evaluate(&query.clone().encode().unwrap())
        .unwrap();
    let right = gestures.phones()[1]
        .energy()
        .evaluate(&query.clone().encode().unwrap())
        .unwrap();
    assert_eq!(left.result(), right.result());
    let AudioTrajectoryQuantity::Amplitude(value) = left.result() else {
        panic!("amplitude")
    };
    assert_eq!(*value.numerator() * 4, *value.denominator() * 3);
    // Authored2/4→4/4 energy actually changes across this same-owner overlap.
    assert_ne!(
        left.original().segments()[2].left(),
        left.original().segments()[2].right()
    );
    for g in gestures.phones() {
        let selected = gestures
            .selected(g)
            .unwrap()
            .definition()
            .identity()
            .get()
            .as_str();
        let expected = match selected {
            "g1/aspirated-t" => [0, 1, 1],
            "g1/t" => [0, 1, 0],
            "g1/a" => [1, 1, 1],
            _ => panic!(),
        };
        for (phase, value) in [g.closure(), g.release(), g.aspiration()]
            .into_iter()
            .zip(expected)
        {
            assert_eq!(
                phase.left(),
                &AudioTrajectoryQuantity::amplitude(1, value).unwrap()
            );
            assert_eq!(phase.left(), phase.right());
        }
        assert_eq!(g.closure().start(), g.phases.nominal_start());
        assert_eq!(g.closure().end(), g.phases.closure_end());
        assert_eq!(g.release().start(), g.phases.closure_end());
        assert_eq!(g.release().end(), g.phases.release_end());
        assert_eq!(g.aspiration().start(), g.phases.release_end());
        assert_eq!(g.aspiration().end(), g.phases.nominal_end());
    }
    let foreign_anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(2).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let foreign_query =
        AudioTrajectoryQuery::new(foreign_anchor, first.phases.leading_end().clone()).unwrap();
    assert!(matches!(
        first.resonance()[0].evaluate(&foreign_query.clone().encode().unwrap()),
        Err(AudioTrajectoryRefusal::ForeignAnchor)
    ));
    let foreign_denominator =
        AudioExactTimeOffset::new(601, *first.phases.leading_end().numerator_seconds()).unwrap();
    let query = AudioTrajectoryQuery::new(anchor(), foreign_denominator).unwrap();
    assert!(first.resonance()[0]
        .evaluate(&query.clone().encode().unwrap())
        .is_err());
    for rate in [8000, 48000] {
        let projected = conduit_speech::timing::duration_at_rate(
            &SpeechDurationAtRateRequest::new(first.timing.duration().clone(), rate).unwrap(),
        )
        .unwrap();
        assert_eq!(projected.request().duration(), first.timing.duration());
    }
    assert!(gestures
        .executions()
        .iter()
        .all(|e| !e.source_id().is_empty()
            && !e.program_hex().is_empty()
            && !e.input_canonical().is_empty()
            && !e.output_canonical().is_empty()));
}
#[test]
fn explicit_boundary_stops_overlap_and_contributes_exact_semantic_time() {
    let f = Fixture::new(true);
    let owner = f.owner();
    let controls = controls(&owner);
    let profile = PhoneGestureProfile::prepare(&owner, "G2/boundary@1", &controls).unwrap();
    let gestures =
        prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), capacity()).unwrap();
    let before = &gestures.phones()[3];
    let after = &gestures.phones()[4];
    assert_eq!(before.next_segment, None);
    assert_eq!(after.previous_segment, None);
    assert_eq!(before.phases.trailing_end(), before.phases.nominal_end());
    assert_eq!(after.phases.leading_start(), after.phases.nominal_start());
    assert_eq!(*after.timing.start().denominator(), 30);
    assert_eq!(*after.timing.start().numerator_seconds(), 5);
}
#[test]
fn foreign_owner_missing_duplicate_control_and_logical_pressure_cannot_prepare() {
    let f = Fixture::new(false);
    let owner = f.owner();
    let other = f.owner();
    let controls = controls(&owner);
    let profile = PhoneGestureProfile::prepare(&owner, "G2@1", &controls).unwrap();
    assert!(matches!(
        prepare_phone_gestures(&other, &profile, &anchor(), &provenance(), capacity()),
        Err(PhoneGestureRefusal::ForeignOwner)
    ));
    assert!(matches!(
        PhoneGestureProfile::prepare(&owner, "G2@1", &controls[..1]),
        Err(PhoneGestureRefusal::MissingControl)
    ));
    let duplicate = [
        PhoneGestureControl {
            definition: controls[0].definition,
            resonance: controls[0].resonance.clone(),
            energy: controls[0].energy.clone(),
            closure: controls[0].closure.clone(),
            release: controls[0].release.clone(),
            aspiration: controls[0].aspiration.clone(),
        },
        PhoneGestureControl {
            definition: controls[0].definition,
            resonance: controls[0].resonance.clone(),
            energy: controls[0].energy.clone(),
            closure: controls[0].closure.clone(),
            release: controls[0].release.clone(),
            aspiration: controls[0].aspiration.clone(),
        },
    ];
    assert!(matches!(
        PhoneGestureProfile::prepare(&owner, "G2@1", &duplicate),
        Err(PhoneGestureRefusal::DuplicateControl)
    ));
    let foreign = controls[0].definition.clone();
    let substituted = [PhoneGestureControl {
        definition: &foreign,
        resonance: controls[0].resonance.clone(),
        energy: controls[0].energy.clone(),
        closure: controls[0].closure.clone(),
        release: controls[0].release.clone(),
        aspiration: controls[0].aspiration.clone(),
    }];
    assert!(matches!(
        PhoneGestureProfile::prepare(&owner, "G2@1", &substituted),
        Err(PhoneGestureRefusal::ForeignDefinition)
    ));
    for cap in [
        PhoneGestureCapacity {
            events: 1,
            ..capacity()
        },
        PhoneGestureCapacity {
            trajectories: 1,
            ..capacity()
        },
        PhoneGestureCapacity {
            source_executions: 1,
            ..capacity()
        },
    ] {
        assert!(matches!(
            prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), cap),
            Err(PhoneGestureRefusal::Capacity)
        ));
    }
}

#[test]
fn missing_domain_and_overbound_authored_controls_refuse_without_changing_source() {
    let f = Fixture::new(false);
    let owner = f.owner();
    let original = f.intent.clone().encode().unwrap();
    let mut wrong = controls(&owner);
    wrong[0].resonance[0] = AudioTrajectoryQuantity::amplitude(1, 1).unwrap();
    assert!(matches!(
        PhoneGestureProfile::prepare(&owner, "G2@1", &wrong),
        Err(PhoneGestureRefusal::Domain)
    ));
    let mut overbound = controls(&owner);
    overbound[0].resonance[0] = AudioTrajectoryQuantity::frequency(1, 1_u64 << 40).unwrap();
    let profile = PhoneGestureProfile::prepare(&owner, "G2/overbound@1", &overbound).unwrap();
    assert!(matches!(
        prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), capacity()),
        Err(PhoneGestureRefusal::Audio(_))
    ));
    assert_eq!(f.intent.clone().encode().unwrap(), original);
}

#[test]
fn foreign_exact_time_basis_refuses_before_any_g2_owner_is_returned() {
    // Equal elapsed duration, supported by G1's existing frame-grid admission.
    // G2 nevertheless refuses a foreign exact denominator without normalizing.
    for (denominator, numerator) in [(60, 2)] {
        let mut f = Fixture::new(false);
        f.replace_duration(2, denominator, numerator);
        let owner = f.owner();
        let controls = controls(&owner);
        let profile = PhoneGestureProfile::prepare(&owner, "G2/bad-time@1", &controls).unwrap();
        assert!(matches!(
            prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), capacity()),
            Err(PhoneGestureRefusal::InvalidDuration { .. })
        ));
    }
}

#[test]
fn unequal_neighbor_durations_are_taken_from_the_same_original_occurrences() {
    let mut f = Fixture::new(false);
    f.replace_duration(1, 30, 2);
    let owner = f.owner();
    let controls = controls(&owner);
    let profile = PhoneGestureProfile::prepare(&owner, "G2/unequal@1", &controls).unwrap();
    let gestures =
        prepare_phone_gestures(&owner, &profile, &anchor(), &provenance(), capacity()).unwrap();
    let first = &gestures.phones()[0];
    let middle = &gestures.phones()[1];
    let next = &gestures.phones()[2];
    assert_eq!(
        *first.timing.next().as_ref().unwrap().numerator_seconds(),
        2
    );
    assert_eq!(*middle.timing.duration().numerator_seconds(), 2);
    assert_eq!(
        *middle
            .timing
            .previous()
            .as_ref()
            .unwrap()
            .numerator_seconds(),
        1
    );
    assert_eq!(
        *next.timing.previous().as_ref().unwrap().numerator_seconds(),
        2
    );
    assert_eq!(first.phases.trailing_start(), middle.phases.leading_start());
    assert_eq!(first.phases.trailing_end(), middle.phases.leading_end());
    assert_eq!(middle.phases.trailing_start(), next.phases.leading_start());
    assert_eq!(middle.phases.trailing_end(), next.phases.leading_end());
    assert_eq!(*middle.phases.leading_start().numerator_seconds(), 15);
    assert_eq!(*middle.phases.leading_end().numerator_seconds(), 30);
    assert_eq!(*next.timing.start().numerator_seconds(), 3);
    assert_eq!(*next.timing.start().denominator(), 30);
}
