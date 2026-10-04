#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{output_features::*, semantic::*};
fn bundle(entries: &[(&str, FeatureSpecification)]) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter(entries.iter().map(|(id, spec)| {
            SpeechFeature::new(SpeechFeatureId::new((*id).into()).unwrap(), spec.clone()).unwrap()
        }))
        .unwrap(),
    )
    .unwrap()
}
fn known(value: bool) -> FeatureSpecification {
    FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap()
}
fn rule(phone: PhoneSpecification, output: SpeechFeatureBundle) -> SpeechAllophoneRule {
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
        "original rule".into(),
        bundle(&[]),
        output,
        phone,
        PhonemeSpecification::unknown(),
        SpeechRuleStatus::Experimental,
    )
    .unwrap()
}
fn phone(id: &str, features: SpeechFeatureBundle) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        features,
        PhoneId::new(id.into()).unwrap(),
        "t".into(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
fn phone_spec() -> PhoneSpecification {
    PhoneSpecification::known(PhoneId::new("phone/t".into()).unwrap()).unwrap()
}
#[test]
fn later_layers_override_entire_specifications_and_preserve_original_references() {
    let default = bundle(&[("voice", known(true)), ("default", known(true))]);
    let definition = phone(
        "phone/t",
        bundle(&[("voice", known(false)), ("phone", known(true))]),
    );
    let rule = rule(
        phone_spec(),
        bundle(&[
            ("voice", FeatureSpecification::unknown()),
            ("rule", known(true)),
        ]),
    );
    let receipt = prepare_rule_output_features(&rule, Some(&default), Some(&definition)).unwrap();
    assert!(core::ptr::eq(receipt.rule(), &rule));
    assert!(core::ptr::eq(receipt.default_features().unwrap(), &default));
    assert!(core::ptr::eq(
        receipt.phone_definition().unwrap(),
        &definition
    ));
    assert!(receipt.checked_phone().is_some());
    let entries: Vec<_> = receipt.features().collect();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].layer(), OutputFeatureLayer::RuleOutput);
    assert!(core::ptr::eq(
        entries[0].feature(),
        &rule.output_features().get().as_slice()[0]
    ));
    assert_eq!(
        entries[0].feature().specification(),
        &FeatureSpecification::unknown()
    );
    assert_eq!(entries[1].layer(), OutputFeatureLayer::DefaultRealization);
    assert_eq!(entries[2].layer(), OutputFeatureLayer::PhoneDefinition);
    assert_eq!(entries[3].layer(), OutputFeatureLayer::RuleOutput);
}
#[test]
fn missing_layers_stay_missing_and_nonknown_phones_do_not_inherit_defaults() {
    let default = bundle(&[("default", known(true))]);
    let states = [
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(
            BoundedSequence::try_from_iter([PhoneId::new("phone/t".into()).unwrap()]).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            PhoneId::new("phone/t".into()).unwrap(),
        )
        .unwrap(),
    ];
    for state in states {
        let rule = rule(
            state,
            bundle(&[("rule", FeatureSpecification::not_applicable())]),
        );
        let receipt = prepare_rule_output_features(&rule, Some(&default), None).unwrap();
        assert!(receipt.phone_definition().is_none());
        assert!(receipt.checked_phone().is_none());
        assert_eq!(receipt.features().count(), 1);
        assert_eq!(
            receipt.features().next().unwrap().layer(),
            OutputFeatureLayer::RuleOutput
        );
    }
    let rule = rule(phone_spec(), bundle(&[]));
    let receipt = prepare_rule_output_features(&rule, None, None).unwrap();
    assert!(receipt.default_features().is_none());
    assert!(receipt.phone_definition().is_none());
    assert_eq!(receipt.features().count(), 0);
}
#[test]
fn wrong_phone_duplicate_layers_and_overflow_refuse_without_partial_output() {
    let rule = rule(phone_spec(), bundle(&[]));
    let wrong = phone("phone/other", bundle(&[]));
    assert!(matches!(
        prepare_rule_output_features(&rule, None, Some(&wrong)),
        Err(OutputFeatureRefusal::PhoneIdentity(_))
    ));
    let unknown = self::rule(PhoneSpecification::unknown(), bundle(&[]));
    assert!(matches!(
        prepare_rule_output_features(&unknown, None, Some(&wrong)),
        Err(OutputFeatureRefusal::UnexpectedPhoneDefinition)
    ));
    let duplicate = bundle(&[("voice", known(true)), ("voice", known(false))]);
    assert!(matches!(
        prepare_rule_output_features(&rule, Some(&duplicate), None),
        Err(OutputFeatureRefusal::Bundle {
            layer: OutputFeatureLayer::DefaultRealization,
            ..
        })
    ));
    let duplicate_phone = phone("phone/t", duplicate.clone());
    assert!(matches!(
        prepare_rule_output_features(&rule, None, Some(&duplicate_phone)),
        Err(OutputFeatureRefusal::Bundle {
            layer: OutputFeatureLayer::PhoneDefinition,
            ..
        })
    ));
    let duplicate_output = self::rule(phone_spec(), duplicate);
    assert!(matches!(
        prepare_rule_output_features(&duplicate_output, None, None),
        Err(OutputFeatureRefusal::Bundle {
            layer: OutputFeatureLayer::RuleOutput,
            ..
        })
    ));
    let keys: Vec<_> = (0..16).map(|n| format!("feature/{n}")).collect();
    let full = bundle(
        &keys
            .iter()
            .map(|key| (key.as_str(), known(true)))
            .collect::<Vec<_>>(),
    );
    let receipt = prepare_rule_output_features(&rule, Some(&full), None).unwrap();
    assert_eq!(receipt.features().count(), 16);
    let extra = self::rule(phone_spec(), bundle(&[("extra", known(true))]));
    assert!(matches!(
        prepare_rule_output_features(&extra, Some(&full), None),
        Err(OutputFeatureRefusal::Capacity)
    ));
    let overrides = self::rule(phone_spec(), full.clone());
    assert_eq!(
        prepare_rule_output_features(&overrides, Some(&full), None)
            .unwrap()
            .features()
            .count(),
        16
    );
}

#[test]
fn every_rule_feature_specification_replaces_known_lower_layers_without_resolution() {
    let value = SpeechFeatureValue::boolean(false).unwrap();
    let states = [
        FeatureSpecification::known(value.clone()).unwrap(),
        FeatureSpecification::unknown(),
        FeatureSpecification::unspecified(),
        FeatureSpecification::not_applicable(),
        FeatureSpecification::variable(BoundedSequence::try_from_iter([value.clone()]).unwrap())
            .unwrap(),
        FeatureSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            value,
        )
        .unwrap(),
    ];
    let default = bundle(&[("voice", known(true))]);
    let definition = phone("phone/t", bundle(&[("voice", known(true))]));
    for specification in states {
        let rule = rule(phone_spec(), bundle(&[("voice", specification.clone())]));
        let receipt =
            prepare_rule_output_features(&rule, Some(&default), Some(&definition)).unwrap();
        let feature = receipt.features().next().unwrap();
        assert_eq!(feature.layer(), OutputFeatureLayer::RuleOutput);
        assert_eq!(feature.feature().specification(), &specification);
        assert!(core::ptr::eq(
            feature.feature(),
            &rule.output_features().get().as_slice()[0]
        ));
    }
}
