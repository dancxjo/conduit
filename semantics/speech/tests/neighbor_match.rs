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
        },
    )
    .unwrap();
    assert_eq!(compared.decision(), &NeighborDecision::Mismatched);
}

#[test]
fn any_requires_observed_neighbor_and_features_remain_unsupported() {
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
                phoneme: &unknown_phoneme
            }
        )
        .unwrap()
        .decision(),
        &NeighborDecision::Matched
    );
    let features =
        SpeechSegmentMatcher::features(SpeechFeatureBundle::new(BoundedSequence::new()).unwrap())
            .unwrap();
    for observation in [
        NeighborObservation::Absent,
        NeighborObservation::Unknown,
        NeighborObservation::Segment {
            phone: &unknown_phone,
            phoneme: &unknown_phoneme,
        },
    ] {
        assert_eq!(
            compare_neighbor(&features, observation).unwrap().decision(),
            &NeighborDecision::UnsupportedMatcher
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
        (3, NeighborDecision::UnsupportedMatcher),
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
