#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{rule_input::*, semantic::*};
macro_rules! state_test {
    ($test:ident,$compare:ident,$spec:ident,$id:ident) => {
        #[test]
        fn $test() {
            let states = |value: &str| {
                let id = $id::new(value.into()).unwrap();
                [
                    $spec::known(id.clone()).unwrap(),
                    $spec::unknown(),
                    $spec::unspecified(),
                    $spec::not_applicable(),
                    $spec::variable(BoundedSequence::try_from_iter([id.clone()]).unwrap()).unwrap(),
                    $spec::gradient(SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(), id)
                        .unwrap(),
                ]
            };
            let requirements = states("opaque/t");
            for (same, observations) in [(true, states("opaque/t")), (false, states("opaque/T"))] {
                for (r, requirement) in requirements.iter().enumerate() {
                    for (o, observation) in observations.iter().enumerate() {
                        let receipt = $compare(requirement, observation).unwrap();
                        let expected = if r == 2 {
                            SpeechContextDecision::Matched
                        } else if r != 0 {
                            SpeechContextDecision::RequirementUnresolved
                        } else if o != 0 {
                            SpeechContextDecision::ObservationUnresolved
                        } else if same {
                            SpeechContextDecision::Matched
                        } else {
                            SpeechContextDecision::Mismatched
                        };
                        assert_eq!(receipt.decision(), &expected);
                        assert!(core::ptr::eq(receipt.requirement(), requirement));
                        assert!(core::ptr::eq(receipt.observation(), observation));
                        if r == 0 && o == 0 {
                            assert_eq!(receipt.identity().unwrap().is_ok(), same);
                        } else {
                            assert!(receipt.identity().is_none());
                        }
                    }
                }
            }
        }
    };
}
state_test!(
    phoneme_states_keep_exact_original_patterns,
    compare_phoneme_pattern,
    PhonemeSpecification,
    PhonemeId
);
state_test!(
    phone_states_keep_exact_original_patterns,
    compare_phone_pattern,
    PhoneSpecification,
    PhoneId
);
#[test]
fn identity_is_exact_utf8_without_notation_normalization_or_base_id_fallback() {
    for (required, observed, same) in [
        ("é", "é", true),
        ("é", "e\u{301}", false),
        ("t", "t/primary", false),
        ("T", "t", false),
    ] {
        let required =
            PhonemeSpecification::known(PhonemeId::new(required.into()).unwrap()).unwrap();
        let observed =
            PhonemeSpecification::known(PhonemeId::new(observed.into()).unwrap()).unwrap();
        let receipt = compare_phoneme_pattern(&required, &observed).unwrap();
        assert_eq!(
            receipt.decision(),
            if same {
                &SpeechContextDecision::Matched
            } else {
                &SpeechContextDecision::Mismatched
            }
        );
    }
    assert_ne!(
        core::any::TypeId::of::<PhoneId>(),
        core::any::TypeId::of::<PhonemeId>()
    );
    assert_ne!(
        SpeechPhonePatternIdentity::semantic_type(),
        SpeechPhonemePatternIdentity::semantic_type()
    );
}
fn feature(value: FeatureSpecification) -> SpeechFeature {
    SpeechFeature::new(SpeechFeatureId::new("voiced".into()).unwrap(), value).unwrap()
}
fn bundle(features: Vec<SpeechFeature>) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(BoundedSequence::try_from_iter(features).unwrap()).unwrap()
}
fn fixture_rule(phoneme: PhonemeSpecification, input: SpeechFeatureBundle) -> SpeechAllophoneRule {
    SpeechAllophoneRule::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        "original global rule".into(),
        input,
        bundle(vec![]),
        PhoneSpecification::unknown(),
        phoneme,
        SpeechRuleStatus::Experimental,
    )
    .unwrap()
}
fn known(id: &str) -> PhonemeSpecification {
    PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap()
}
#[test]
fn whole_rule_input_retains_both_components_without_inference_or_selection() {
    let rule = fixture_rule(
        known("t"),
        bundle(vec![feature(
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        )]),
    );
    let observed = known("t");
    let features = bundle(vec![feature(
        FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
    )]);
    let receipt = compare_allophone_rule_input(&rule, &observed, Some(&features)).unwrap();
    assert!(core::ptr::eq(receipt.rule(), &rule));
    assert!(core::ptr::eq(
        receipt.phoneme().requirement(),
        rule.phoneme()
    ));
    assert!(core::ptr::eq(receipt.phoneme().observation(), &observed));
    assert!(core::ptr::eq(
        receipt.features().requirements(),
        rule.input_features()
    ));
    assert!(core::ptr::eq(
        receipt.features().observations().unwrap(),
        &features
    ));
    assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
    assert!(matches!(
        receipt.rule().phone(),
        PhoneSpecification::Unknown
    ));
    assert_eq!(receipt.rule().status(), &SpeechRuleStatus::Experimental);
    let absent = compare_allophone_rule_input(&rule, &observed, None).unwrap();
    assert_eq!(
        absent.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert!(absent.features().observations().is_none());
    assert_eq!(absent.features().comparisons().count(), 1);
}
#[test]
fn mismatch_retains_uncertainty_and_requirement_uncertainty_precedes_observation_uncertainty() {
    let rule = fixture_rule(
        known("t"),
        bundle(vec![feature(
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        )]),
    );
    let wrong = known("d");
    let receipt = compare_allophone_rule_input(&rule, &wrong, None).unwrap();
    assert_eq!(receipt.decision(), &SpeechContextDecision::Mismatched);
    assert_eq!(
        receipt.features().decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert!(receipt.phoneme().identity().unwrap().is_err());
    let unresolved = PhonemeSpecification::unknown();
    let receipt = compare_allophone_rule_input(&rule, &unresolved, None).unwrap();
    assert_eq!(
        receipt.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    let uncertain_rule = rule_with_unknown_pattern();
    let receipt = compare_allophone_rule_input(&uncertain_rule, &wrong, None).unwrap();
    assert_eq!(
        receipt.decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    let wildcard = fixture_rule(PhonemeSpecification::unspecified(), bundle(vec![]));
    assert_eq!(
        compare_allophone_rule_input(&wildcard, &unresolved, None)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Matched
    );
}
fn rule_with_unknown_pattern() -> SpeechAllophoneRule {
    fixture_rule(
        PhonemeSpecification::unknown(),
        bundle(vec![feature(
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        )]),
    )
}
#[test]
fn invalid_feature_evidence_refuses_even_after_phoneme_mismatch() {
    let rule = fixture_rule(known("t"), bundle(vec![]));
    let wrong = known("d");
    let invalid = bundle(vec![
        feature(FeatureSpecification::unknown()),
        feature(FeatureSpecification::unknown()),
    ]);
    assert!(matches!(
        compare_allophone_rule_input(&rule, &wrong, Some(&invalid)),
        Err(RuleInputRefusal::Features(_))
    ));
}
