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
