#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{neighbor_match::*, rule_features::*, semantic::*};
#[test]
fn feature_conditions_keep_original_key_value_and_exact_neighbor_side() {
    let required = SpeechFeatureCondition::new(
        SpeechFeatureId::new("voice".into()).unwrap(),
        SpeechFeatureValue::boolean(true).unwrap(),
    )
    .unwrap();
    let previous = SpeechRuleCondition::previous_has_feature(
        required.identity().clone(),
        required.value().clone(),
    )
    .unwrap();
    let next = SpeechRuleCondition::next_has_feature(
        required.identity().clone(),
        required.value().clone(),
    )
    .unwrap();
    let known = FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap();
    let bundle = SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([
            SpeechFeature::new(
                SpeechFeatureId::new("extra".into()).unwrap(),
                FeatureSpecification::unknown(),
            )
            .unwrap(),
            SpeechFeature::new(SpeechFeatureId::new("voice".into()).unwrap(), known).unwrap(),
        ])
        .unwrap(),
    )
    .unwrap();
    let phone = PhoneSpecification::unknown();
    let phoneme = PhonemeSpecification::unknown();
    let observed = NeighborObservation::Segment {
        phone: &phone,
        phoneme: &phoneme,
        features: Some(&bundle),
    };
    let receipt =
        compare_feature_condition(&previous, observed, NeighborObservation::Absent).unwrap();
    assert!(core::ptr::eq(receipt.condition(), &previous));
    assert!(core::ptr::eq(
        receipt.feature().unwrap(),
        &bundle.get().as_slice()[1]
    ));
    assert!(receipt.checked_key().is_some());
    assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
    assert_eq!(
        compare_feature_condition(&next, observed, NeighborObservation::Absent)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Mismatched
    );
    assert_eq!(
        compare_feature_condition(&next, NeighborObservation::Absent, observed)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Matched
    );
    let missing = NeighborObservation::Segment {
        phone: &phone,
        phoneme: &phoneme,
        features: None,
    };
    let receipt = compare_feature_condition(&previous, missing, observed).unwrap();
    assert!(receipt.feature().is_none());
    assert!(receipt.checked_key().is_none());
    assert_eq!(
        receipt.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(
        compare_feature_condition(&previous, NeighborObservation::Unknown, observed)
            .unwrap()
            .decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert!(matches!(
        compare_feature_condition(
            &SpeechRuleCondition::not_careful_style(),
            observed,
            observed
        ),
        Err(FeatureConditionRefusal::WrongCondition)
    ));
}
