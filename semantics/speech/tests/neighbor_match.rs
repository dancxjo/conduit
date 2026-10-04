#![cfg(feature = "semantic-bindings")]
use conduit_speech::{neighbor_match::*, semantic::*};
fn phone(value: &str) -> PhoneId {
    PhoneId::new(value.into()).unwrap()
}
fn phoneme(value: &str) -> PhonemeId {
    PhonemeId::new(value.into()).unwrap()
}
#[test]
fn exact_neighbor_identity_and_missing_evidence_remain_distinct() {
    let requirement = SpeechSegmentMatcher::phone(phone("phone/t")).unwrap();
    let unknown = PhonemeSpecification::unknown();
    for (value, expected) in [
        ("phone/t", NeighborDecision::Matched),
        ("phone/d", NeighborDecision::Mismatched),
    ] {
        let actual = PhoneSpecification::known(phone(value)).unwrap();
        let compared = compare_neighbor(
            &requirement,
            NeighborObservation::Segment {
                phone: &actual,
                phoneme: &unknown,
                features: None,
            },
        )
        .unwrap();
        assert!(core::ptr::eq(compared.requirement(), &requirement));
        assert_eq!(compared.decision(), &expected);
    }
    let absent = compare_neighbor(&requirement, NeighborObservation::Absent).unwrap();
    assert_eq!(absent.decision(), &NeighborDecision::Mismatched);
    assert!(matches!(absent.observation(), NeighborObservation::Absent));
    let missing = compare_neighbor(&requirement, NeighborObservation::Unknown).unwrap();
    assert_eq!(missing.decision(), &NeighborDecision::ObservationUnresolved);
    assert!(matches!(
        missing.observation(),
        NeighborObservation::Unknown
    ));
}
#[test]
fn phoneme_comparison_never_substitutes_same_spelled_phone() {
    let requirement = SpeechSegmentMatcher::phoneme(phoneme("t")).unwrap();
    let actual_phone = PhoneSpecification::known(phone("t")).unwrap();
    let actual_phoneme = PhonemeSpecification::known(phoneme("d")).unwrap();
    let compared = compare_neighbor(
        &requirement,
        NeighborObservation::Segment {
            phone: &actual_phone,
            phoneme: &actual_phoneme,
            features: None,
        },
    )
    .unwrap();
    assert_eq!(compared.decision(), &NeighborDecision::Mismatched);
}

#[test]
fn any_requires_observed_neighbor_and_empty_features_are_unconstrained() {
    use conduit_plot::rust_binding::BoundedSequence;
    let any = SpeechSegmentMatcher::any();
    let unknown_phone = PhoneSpecification::unknown();
    let unknown_phoneme = PhonemeSpecification::unknown();
    assert_eq!(
        compare_neighbor(&any, NeighborObservation::Absent)
            .unwrap()
            .decision(),
        &NeighborDecision::Mismatched
    );
    assert_eq!(
        compare_neighbor(&any, NeighborObservation::Unknown)
            .unwrap()
            .decision(),
        &NeighborDecision::ObservationUnresolved
    );
    assert_eq!(
        compare_neighbor(
            &any,
            NeighborObservation::Segment {
                phone: &unknown_phone,
                phoneme: &unknown_phoneme,
                features: None,
            }
        )
        .unwrap()
        .decision(),
        &NeighborDecision::Matched
    );
    let features =
        SpeechSegmentMatcher::features(SpeechFeatureBundle::new(BoundedSequence::new()).unwrap())
            .unwrap();
    for (observation, expected) in [
        (NeighborObservation::Absent, NeighborDecision::Mismatched),
        (
            NeighborObservation::Unknown,
            NeighborDecision::ObservationUnresolved,
        ),
        (
            NeighborObservation::Segment {
                phone: &unknown_phone,
                phoneme: &unknown_phoneme,
                features: None,
            },
            NeighborDecision::Matched,
        ),
    ] {
        assert_eq!(
            compare_neighbor(&features, observation).unwrap().decision(),
            &expected
        );
    }
}

#[test]
fn bounded_alternatives_share_one_neighbor_and_preserve_every_receipt() {
    use conduit_plot::rust_binding::BoundedSequence;
    let actual_phone = PhoneSpecification::known(phone("t")).unwrap();
    let actual_phoneme = PhonemeSpecification::unknown();
    let observation = NeighborObservation::Segment {
        phone: &actual_phone,
        phoneme: &actual_phoneme,
        features: None,
    };
    let requirements = [
        SpeechSegmentMatcher::phone(phone("d")).unwrap(),
        SpeechSegmentMatcher::phoneme(phoneme("t")).unwrap(),
        SpeechSegmentMatcher::features(SpeechFeatureBundle::new(BoundedSequence::new()).unwrap())
            .unwrap(),
        SpeechSegmentMatcher::phone(phone("t")).unwrap(),
    ];
    for (count, expected) in [
        (1, NeighborDecision::Mismatched),
        (2, NeighborDecision::ObservationUnresolved),
        (3, NeighborDecision::Matched),
        (4, NeighborDecision::Matched),
    ] {
        let compared = compare_neighbor_alternatives(&requirements[..count], observation).unwrap();
        assert_eq!(compared.decision(), &expected);
        assert_eq!(compared.comparisons().count(), count);
        for (receipt, original) in compared.comparisons().zip(&requirements) {
            assert!(core::ptr::eq(receipt.requirement(), original));
            match receipt.observation() {
                NeighborObservation::Segment { phone, .. } => {
                    assert!(core::ptr::eq(*phone, &actual_phone))
                }
                _ => panic!("original immediate neighbor lost"),
            }
        }
    }
    assert_eq!(
        compare_neighbor_alternatives(&[], NeighborObservation::Absent)
            .unwrap()
            .decision(),
        &NeighborDecision::Matched
    );
    let excessive = core::array::from_fn::<_, 5, _>(|_| SpeechSegmentMatcher::any());
    assert!(matches!(
        compare_neighbor_alternatives(&excessive, observation),
        Err(NeighborComparisonRefusal::TooManyAlternatives { actual: 5 })
    ));
}

#[test]
fn boundary_observations_keep_kind_uncertainty_and_segment_domains_separate() {
    let requirement = SpeechSegmentMatcher::boundary(SpeechBoundaryKind::Word).unwrap();
    let unknown_phone = PhoneSpecification::unknown();
    let unknown_phoneme = PhonemeSpecification::unknown();
    for (kind, expected) in [
        (SpeechBoundaryKind::Word, NeighborDecision::Matched),
        (SpeechBoundaryKind::Phrase, NeighborDecision::Mismatched),
    ] {
        let observation = SpeechBoundarySpecification::known(kind).unwrap();
        let compared =
            compare_neighbor(&requirement, NeighborObservation::Boundary(&observation)).unwrap();
        assert_eq!(compared.decision(), &expected);
        assert!(
            matches!(compared.observation(), NeighborObservation::Boundary(value) if core::ptr::eq(*value, &observation))
        );
    }
    let boundary = SpeechBoundarySpecification::unknown();
    assert_eq!(
        compare_neighbor(&requirement, NeighborObservation::Boundary(&boundary))
            .unwrap()
            .decision(),
        &NeighborDecision::ObservationUnresolved
    );
    assert_eq!(
        compare_neighbor(
            &requirement,
            NeighborObservation::Segment {
                phone: &unknown_phone,
                phoneme: &unknown_phoneme,
                features: None,
            }
        )
        .unwrap()
        .decision(),
        &NeighborDecision::Mismatched
    );
    let phone_requirement = SpeechSegmentMatcher::phone(phone("t")).unwrap();
    assert_eq!(
        compare_neighbor(&phone_requirement, NeighborObservation::Boundary(&boundary))
            .unwrap()
            .decision(),
        &NeighborDecision::Mismatched
    );
}

#[test]
fn all_boundary_kinds_follow_the_exact_native_equality_law() {
    let kinds = [
        SpeechBoundaryKind::Phone,
        SpeechBoundaryKind::Syllable,
        SpeechBoundaryKind::Morpheme,
        SpeechBoundaryKind::Word,
        SpeechBoundaryKind::Phrase,
        SpeechBoundaryKind::BreathGroup,
        SpeechBoundaryKind::Turn,
    ];
    for expected in kinds {
        let requirement = SpeechSegmentMatcher::boundary(expected).unwrap();
        for actual in kinds {
            let observation = SpeechBoundarySpecification::known(actual).unwrap();
            let compared =
                compare_neighbor(&requirement, NeighborObservation::Boundary(&observation))
                    .unwrap();
            assert_eq!(
                compared.decision(),
                if expected == actual {
                    &NeighborDecision::Matched
                } else {
                    &NeighborDecision::Mismatched
                }
            );
        }
    }
}

#[test]
fn feature_neighbors_retain_bundle_evidence_and_distinct_uncertainty() {
    use conduit_plot::rust_binding::BoundedSequence;
    let bundle = |specification| {
        SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([SpeechFeature::new(
                SpeechFeatureId::new("voice".into()).unwrap(),
                specification,
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap()
    };
    let known =
        |value| FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap();
    let observed_true = bundle(known(true));
    let observed_false = bundle(known(false));
    let observed_unknown = bundle(FeatureSpecification::unknown());
    let phone = PhoneSpecification::unknown();
    let phoneme = PhonemeSpecification::unknown();
    let requirement = SpeechSegmentMatcher::features(bundle(known(true))).unwrap();
    for (observed, expected) in [
        (Some(&observed_true), NeighborDecision::Matched),
        (Some(&observed_false), NeighborDecision::Mismatched),
        (
            Some(&observed_unknown),
            NeighborDecision::ObservationUnresolved,
        ),
        (None, NeighborDecision::ObservationUnresolved),
    ] {
        let compared = compare_neighbor(
            &requirement,
            NeighborObservation::Segment {
                phone: &phone,
                phoneme: &phoneme,
                features: observed,
            },
        )
        .unwrap();
        assert_eq!(compared.decision(), &expected);
        let proof = compared.features().unwrap();
        assert_eq!(proof.comparisons().count(), 1);
        assert_eq!(
            proof.observations().map(core::ptr::from_ref),
            observed.map(core::ptr::from_ref)
        );
    }
    let unresolved =
        SpeechSegmentMatcher::features(bundle(FeatureSpecification::unknown())).unwrap();
    assert_eq!(
        compare_neighbor(
            &unresolved,
            NeighborObservation::Segment {
                phone: &phone,
                phoneme: &phoneme,
                features: Some(&observed_true)
            }
        )
        .unwrap()
        .decision(),
        &NeighborDecision::RequirementUnresolved
    );
    let wildcard =
        SpeechSegmentMatcher::features(bundle(FeatureSpecification::unspecified())).unwrap();
    assert_eq!(
        compare_neighbor(
            &wildcard,
            NeighborObservation::Segment {
                phone: &phone,
                phoneme: &phoneme,
                features: None
            }
        )
        .unwrap()
        .decision(),
        &NeighborDecision::Matched
    );
}
