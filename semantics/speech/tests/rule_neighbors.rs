#![cfg(feature = "semantic-bindings")]
use conduit_speech::{neighbor_match::*, rule_neighbors::*, semantic::*};
#[test]
fn conditions_preserve_original_matcher_and_select_exact_immediate_side() {
    let previous = SpeechRuleCondition::previous_matches(
        SpeechSegmentMatcher::phone(PhoneId::new("t".into()).unwrap()).unwrap(),
    )
    .unwrap();
    let next = SpeechRuleCondition::next_matches(
        SpeechSegmentMatcher::phone(PhoneId::new("d".into()).unwrap()).unwrap(),
    )
    .unwrap();
    let phone = PhoneSpecification::known(PhoneId::new("t".into()).unwrap()).unwrap();
    let phoneme = PhonemeSpecification::unknown();
    let before = NeighborObservation::Segment {
        phone: &phone,
        phoneme: &phoneme,
        features: None,
    };
    let after = NeighborObservation::Absent;
    let receipt = compare_neighbor_condition(&previous, before, after).unwrap();
    assert!(core::ptr::eq(receipt.condition(), &previous));
    assert_eq!(receipt.comparison().decision(), &NeighborDecision::Matched);
    if let SpeechRuleCondition::PreviousMatches(matcher) = &previous {
        assert!(core::ptr::eq(receipt.comparison().requirement(), matcher));
    } else {
        panic!("condition")
    }
    let receipt = compare_neighbor_condition(&next, before, after).unwrap();
    assert!(matches!(
        receipt.comparison().observation(),
        NeighborObservation::Absent
    ));
    assert_eq!(
        receipt.comparison().decision(),
        &NeighborDecision::Mismatched
    );
    let receipt = compare_neighbor_condition(&next, before, NeighborObservation::Unknown).unwrap();
    assert_eq!(
        receipt.comparison().decision(),
        &NeighborDecision::ObservationUnresolved
    );
    let wrong = SpeechRuleCondition::not_careful_style();
    assert!(matches!(
        compare_neighbor_condition(&wrong, before, after),
        Err(NeighborConditionRefusal::WrongCondition)
    ));
}
